//! The running wallpaper on a Mac: one borderless window per screen, one level above the system
//! wallpaper and below the desktop icons, plus the frame loop and the menu bar item.
//!
//! Memory rules as on Windows: a wallpaper that no screen shows is unloaded, screens of the same
//! size share one rendered frame, a picture is decoded once at screen size, and when no scene can
//! be seen the render targets are freed and the timer slows down to one tick a second. Videos
//! and pictures live in Core Animation layers and need no ticks at all.

mod menu;
mod video;

use crate::config::{self, Config, PowerRule, BUILTIN_SCENE};
use crate::gpu::{Gpu, Target};
use crate::library::{self, Item, Kind};
use crate::render::{Post, Renderer};
use crate::world::World;
use crate::{autostart, desktop, log, media, os, scene, SCENE_VIGNETTE};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWindowOcclusionState, NSWindowStyleMask};
use objc2_foundation::{NSProcessInfo, NSTimer};
use objc2_quartz_core::{kCAGravityResize, kCAGravityResizeAspect, kCAGravityResizeAspectFill, CAAutoresizingMask, CALayer};
use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use video::Video;

/// Retina screens have four times the pixels; a wallpaper behind other windows does not need them
/// and every target would cost four times the memory.
const RETINA_SCALE: f64 = 0.6;
/// Seconds between two checks for changed screens, power state and videos that failed.
const CHECK_EVERY: Duration = Duration::from_secs(2);
const IDLE_TICK: f64 = 1.0;

const DESKTOP_WINDOW_LEVEL_KEY: i32 = 2;
const TIME_REMAINING_UNLIMITED: f64 = -2.0;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowLevelForKey(key: i32) -> i32;
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPSGetTimeRemainingEstimate() -> f64;
}

fn on_battery() -> bool {
    unsafe { IOPSGetTimeRemainingEstimate() != TIME_REMAINING_UNLIMITED }
}

/// The control panel saved the settings (see `os::notify_engine`).
static RELOAD: AtomicBool = AtomicBool::new(false);

extern "C" fn on_reload(_signal: i32) {
    RELOAD.store(true, Ordering::Relaxed);
}

enum Content {
    Scene { renderer: Renderer, world: World },
    Video(Video),
}

struct Player {
    id: String,
    content: Content,
}

struct Screen {
    window: Retained<NSWindow>,
    /// only for scenes: videos and pictures are layers of the window
    surface: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    monitor: String,
    wallpaper: String,
}

impl Screen {
    fn hidden(&self) -> bool {
        !self.window.occlusionState().contains(NSWindowOcclusionState::Visible)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.surface = None;
        self.window.close();
    }
}

fn create_window(screen: &NSScreen, mtm: MainThreadMarker) -> Retained<NSWindow> {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer_screen(NSWindow::alloc(mtm), screen.frame(), NSWindowStyleMask::Borderless, NSBackingStoreType::Buffered, false, Some(screen))
    };
    unsafe {
        window.setReleasedWhenClosed(false);
        window.setLevel(CGWindowLevelForKey(DESKTOP_WINDOW_LEVEL_KEY) as isize + 1);
    }
    window.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary | NSWindowCollectionBehavior::IgnoresCycle);
    window.setIgnoresMouseEvents(true);
    window.setOpaque(true);
    window.setHasShadow(false);
    window.setBackgroundColor(Some(&NSColor::blackColor()));
    window
}

fn attach_surface(gpu: &Gpu, window: &NSWindow, screen: &NSScreen, quality: f32) -> Result<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration), String> {
    let (frame, scale) = (screen.frame(), screen.backingScaleFactor());
    let view = window.contentView().ok_or("window without a content view")?;
    let handle = AppKitWindowHandle::new(NonNull::from(&*view).cast());
    let target = wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())), raw_window_handle: RawWindowHandle::AppKit(handle) };
    let surface = unsafe { gpu.instance.create_surface_unsafe(target) }.map_err(|e| e.to_string())?;
    let retina = if scale > 1.5 { RETINA_SCALE } else { 1.0 };
    let pixels = |points: f64| ((points * scale * retina * quality as f64) as u32).max(16);
    let mut config = surface.get_default_config(&gpu.adapter, pixels(frame.size.width), pixels(frame.size.height)).ok_or("surface not supported")?;
    // the shaders write sRGB-encoded values themselves, as into a WebGL canvas
    config.format = wgpu::TextureFormat::Bgra8Unorm;
    config.present_mode = wgpu::PresentMode::Fifo;
    config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    config.desired_maximum_frame_latency = 1;
    surface.configure(&gpu.device, &config);
    log(&format!("{}x{} points at scale {scale}: drawing {}x{}", frame.size.width, frame.size.height, config.width, config.height));
    Ok((surface, config))
}

fn attach_layer(window: &NSWindow, layer: &CALayer) -> Result<(), String> {
    let view = window.contentView().ok_or("window without a content view")?;
    view.setWantsLayer(true);
    let root = view.layer().ok_or("view without a layer")?;
    layer.setFrame(view.bounds());
    layer.setAutoresizingMask(CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable);
    root.addSublayer(layer);
    Ok(())
}

fn picture_layer(item: &Item, fit: config::Fit, screen: &NSScreen) -> Result<Retained<CALayer>, String> {
    let (frame, scale) = (screen.frame(), screen.backingScaleFactor());
    let side = (frame.size.width.max(frame.size.height) * scale) as u32;
    let image = media::load_picture(&item.path(), side)?;
    let layer = CALayer::new();
    unsafe {
        let contents: *const objc2_core_graphics::CGImage = &*image;
        layer.setContents(Some(&*contents.cast::<AnyObject>()));
        layer.setContentsGravity(match fit {
            config::Fit::Cover => kCAGravityResizeAspectFill,
            config::Fit::Contain => kCAGravityResizeAspect,
            config::Fit::Stretch => kCAGravityResize,
        });
    }
    layer.setContentsScale(scale);
    Ok(layer)
}

struct Engine {
    gpu: Gpu,
    post: Post,
    players: Vec<Player>,
    cfg: Config,
    screens: Vec<Screen>,
    targets: Vec<Target>,
    /// wallpapers that could not be shown, with the reason (the panel reads them from status.json)
    failed: Vec<(String, String)>,
    /// what the windows were built from: a change means they are built anew
    built_for: String,
    last: Instant,
    next_check: Instant,
    /// nothing is drawn right now and the targets are gone
    idle: bool,
    rule: PowerRule,
    mtm: MainThreadMarker,
}

impl Engine {
    /// What a screen shows: its wallpaper, or the built-in scene if that one failed.
    fn shown_on(&self, monitor: &str) -> String {
        let w = self.cfg.wallpaper_for(monitor);
        if self.failed.iter().any(|(id, _)| id == w) { BUILTIN_SCENE.into() } else { w.to_string() }
    }

    /// Everything the windows depend on; quality changes the surfaces, fit the layers.
    fn plan(&self) -> String {
        let mut plan = format!("{:?}", self.cfg.quality);
        for screen in NSScreen::screens(self.mtm).iter() {
            let (id, f) = (desktop::screen_id(&screen), screen.frame());
            if !self.cfg.off.contains(&id) {
                let wallpaper = self.shown_on(&id);
                plan += &format!("|{id}={wallpaper}:{:?}@{f:?}x{}", self.cfg.media(&wallpaper).fit, screen.backingScaleFactor());
            }
        }
        plan
    }

    fn write_status(&self) {
        if let Ok(text) = serde_json::to_string(&self.failed) {
            let _ = std::fs::write(config::dir().join("status.json"), text);
        }
    }

    fn give_up(&mut self, id: &str, why: &str) {
        log(&format!("wallpaper '{id}' cannot be shown: {why} - showing the built-in scene"));
        self.failed.push((id.to_string(), why.to_string()));
        self.write_status();
    }

    fn load_scene(&mut self, item: &Item) -> Result<(), String> {
        if !self.players.iter().any(|p| p.id == item.id) {
            let bytes = item.scene_bytes().map_err(|e| e.to_string())?;
            let scene = scene::parse(&bytes)?;
            let renderer = Renderer::new(&self.gpu, &scene, scene.manifest.shadow.size);
            let world = World::new(scene.manifest);
            self.players.push(Player { id: item.id.clone(), content: Content::Scene { renderer, world } });
        }
        Ok(())
    }

    fn video(&mut self, item: &Item) -> &Video {
        let speed = self.cfg.media(&item.id).speed;
        if !self.players.iter().any(|p| p.id == item.id) {
            let video = Video::new(&item.path(), speed, self.mtm);
            self.players.push(Player { id: item.id.clone(), content: Content::Video(video) });
        }
        match self.players.iter().find(|p| p.id == item.id).map(|p| &p.content) {
            Some(Content::Video(v)) => v,
            _ => unreachable!("the player was added above"),
        }
    }

    fn show(&mut self, screen: &NSScreen, monitor: &str, wallpaper: &str) -> Result<Screen, String> {
        let item = library::get(wallpaper).ok_or("not in the library")?;
        let window = create_window(screen, self.mtm);
        let fit = self.cfg.media(wallpaper).fit;
        let surface = match item.kind {
            Kind::Scene => {
                self.load_scene(&item)?;
                Some(attach_surface(&self.gpu, &window, screen, self.cfg.quality)?)
            }
            Kind::Video => {
                let layer = self.video(&item).layer(fit);
                attach_layer(&window, &layer)?;
                None
            }
            Kind::Image => {
                let layer = picture_layer(&item, fit, screen)?;
                attach_layer(&window, &layer)?;
                None
            }
        };
        window.orderFrontRegardless();
        Ok(Screen { window, surface, monitor: monitor.to_string(), wallpaper: wallpaper.to_string() })
    }

    fn build_screens(&mut self) {
        self.screens.clear();
        self.targets.clear();
        for screen in NSScreen::screens(self.mtm).iter() {
            let monitor = desktop::screen_id(&screen);
            if self.cfg.off.contains(&monitor) {
                continue;
            }
            let wallpaper = self.shown_on(&monitor);
            match self.show(&screen, &monitor, &wallpaper) {
                Ok(s) => self.screens.push(s),
                Err(e) if wallpaper == BUILTIN_SCENE => log(&format!("the built-in scene could not be loaded: {e}")),
                Err(e) => {
                    self.give_up(&wallpaper, &e);
                    match self.show(&screen, &monitor, BUILTIN_SCENE) {
                        Ok(s) => self.screens.push(s),
                        Err(e) => log(&format!("the built-in scene could not be loaded: {e}")),
                    }
                }
            }
        }
        let shown: Vec<&String> = self.screens.iter().map(|s| &s.wallpaper).collect();
        self.players.retain(|p| shown.contains(&&p.id));
        self.built_for = self.plan();
    }

    /// Re-reads config.json after the control panel changed it.
    fn reload(&mut self) {
        self.cfg = Config::load();
        // a failed wallpaper gets another try once the user has picked something else
        let requested: Vec<String> = self.screens.iter().map(|s| self.cfg.wallpaper_for(&s.monitor).to_string()).collect();
        let before = self.failed.len();
        self.failed.retain(|(id, _)| requested.contains(id));
        if self.failed.len() != before {
            self.write_status();
        }
        if self.plan() != self.built_for {
            self.build_screens();
        }
        for p in &mut self.players {
            if let Content::Video(v) = &mut p.content {
                v.set_speed(self.cfg.media.get(&p.id).map_or(1.0, |m| m.speed));
            }
        }
    }

    fn check(&mut self) {
        if self.plan() != self.built_for {
            log("the screens changed");
            self.build_screens();
        }
        let broken: Option<(String, String)> = self.players.iter().find_map(|p| match &p.content {
            Content::Video(v) => v.error().map(|e| (p.id.clone(), e)),
            _ => None,
        });
        if let Some((id, why)) = broken {
            self.give_up(&id, &why);
            self.build_screens();
        }
        let saver = NSProcessInfo::processInfo().isLowPowerModeEnabled();
        let mut rule = PowerRule::Keep;
        for (active, r) in [(on_battery(), self.cfg.on_battery), (saver, self.cfg.energy_saver)] {
            if active && (r == PowerRule::Pause || (r == PowerRule::Half && rule == PowerRule::Keep)) {
                rule = r;
            }
        }
        self.rule = rule;
    }

    fn interval(&self) -> f64 {
        let fps = self.cfg.fps.max(1) as f64;
        if self.idle {
            IDLE_TICK
        } else if self.rule == PowerRule::Half {
            2.0 / fps
        } else {
            1.0 / fps
        }
    }

    fn wanted(&self) -> Vec<bool> {
        let stopped = self.cfg.paused || self.rule == PowerRule::Pause;
        self.screens.iter().map(|s| !stopped && !(self.cfg.pause_when_covered && s.hidden())).collect()
    }

    /// One tick of the timer; true when the tick speed has to change.
    fn tick(&mut self) -> bool {
        let now = Instant::now();
        let speed = self.interval();
        if RELOAD.swap(false, Ordering::Relaxed) {
            self.reload();
        }
        if now >= self.next_check {
            self.next_check = now + CHECK_EVERY;
            self.check();
        }
        let wanted = self.wanted();
        for p in &mut self.players {
            if let Content::Video(v) = &mut p.content {
                v.set_playing(self.screens.iter().zip(&wanted).any(|(s, w)| *w && s.wallpaper == p.id));
            }
        }
        let drawing = self.screens.iter().zip(&wanted).any(|(s, w)| *w && s.surface.is_some());
        if !drawing {
            if !self.idle {
                self.idle = true;
                self.targets.clear();
                for p in &mut self.players {
                    if let Content::Scene { renderer, .. } = &mut p.content {
                        renderer.release();
                    }
                }
            }
            self.last = now;
            return self.interval() != speed;
        }
        self.idle = false;
        let dt = (now - self.last).as_secs_f32().min(0.25);
        self.last = now;
        for p in &mut self.players {
            let Content::Scene { renderer, world } = &mut p.content else { continue };
            let shown: Vec<usize> = self.screens.iter().enumerate().filter(|(i, s)| wanted[*i] && s.wallpaper == p.id).map(|(i, _)| i).collect();
            if shown.is_empty() {
                continue;
            }
            world.update(dt, &self.cfg.scene(&p.id));
            renderer.prepare(&self.gpu, world);
            let mut rendered: Vec<(u32, u32)> = Vec::new();
            for i in shown {
                let s = &self.screens[i];
                let Some((surface, config)) = &s.surface else { continue };
                let size = (config.width, config.height);
                let at = match self.targets.iter().position(|t| (t.w, t.h) == size) {
                    Some(at) => at,
                    None => {
                        self.targets.push(Target::new(&self.gpu, size.0, size.1));
                        self.targets.len() - 1
                    }
                };
                // the first screen of a size renders, the others show the same frame
                if !rendered.contains(&size) {
                    rendered.push(size);
                    let view = world.view(size.0 as f32 / size.1 as f32, false);
                    renderer.render(&self.gpu, world, &view, &self.targets[at]);
                }
                let frame = match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    _ => {
                        surface.configure(&self.gpu.device, config);
                        continue;
                    }
                };
                let view = frame.texture.create_view(&Default::default());
                self.post.draw(&self.gpu, &self.targets[at], &view, self.cfg.fxaa, SCENE_VIGNETTE);
                self.gpu.queue.present(frame);
            }
        }
        self.interval() != speed
    }
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<Retained<NSTimer>>> = const { RefCell::new(None) };
}

fn schedule(interval: f64) {
    let block = RcBlock::new(|_timer: NonNull<NSTimer>| {
        let change = ENGINE.with_borrow_mut(|e| e.as_mut().and_then(|e| e.tick().then(|| e.interval())));
        if let Some(interval) = change {
            schedule(interval);
        }
    });
    let timer = unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, true, &block) };
    if let Some(old) = TIMER.replace(Some(timer)) {
        old.invalidate();
    }
}

/// The menu bar item asks for this: stop or continue drawing, kept in the settings.
fn toggle_pause() -> bool {
    ENGINE.with_borrow_mut(|e| {
        let Some(e) = e else { return false };
        e.cfg = Config::load();
        e.cfg.paused = !e.cfg.paused;
        e.cfg.save();
        e.next_check = Instant::now();
        e.cfg.paused
    })
}

fn start(cfg: Config, mtm: MainThreadMarker) -> Result<f64, String> {
    let gpu = Gpu::new()?;
    log(&format!("graphics: {}", gpu.name()));
    let post = Post::new(&gpu, wgpu::TextureFormat::Bgra8Unorm);
    let now = Instant::now();
    let mut engine = Engine {
        gpu,
        post,
        players: Vec::new(),
        cfg,
        screens: Vec::new(),
        targets: Vec::new(),
        failed: Vec::new(),
        built_for: String::new(),
        last: now,
        next_check: now + CHECK_EVERY,
        idle: false,
        rule: PowerRule::Keep,
        mtm,
    };
    engine.write_status();
    engine.build_screens();
    engine.check();
    let interval = engine.interval();
    ENGINE.with_borrow_mut(|e| *e = Some(engine));
    Ok(interval)
}

pub fn run(cfg: Config) {
    // the first start ever has nothing to look at but the panel
    let first_run = !config::config_path().exists();
    let Some(lock) = os::try_lock(os::ENGINE_LOCK) else {
        os::open_panel(false);
        return log("already running");
    };
    std::mem::forget(lock);
    os::engine_started();
    autostart::refresh();
    unsafe { libc::signal(libc::SIGUSR1, on_reload as *const () as usize) };
    let mtm = MainThreadMarker::new().expect("the engine runs on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // a menu bar item, but no Dock icon and no menu bar of its own
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let paused = cfg.paused;
    if first_run {
        cfg.save();
    }
    match start(cfg, mtm) {
        Ok(interval) => {
            schedule(interval);
            menu::install(&app, paused, toggle_pause, mtm);
            if first_run {
                os::open_panel(true);
            }
            app.run();
        }
        Err(e) => log(&format!("no graphics device: {e}")),
    }
}

//! The running wallpaper on a Mac: one borderless window per screen, one level above the system
//! wallpaper and below the desktop icons, plus the frame loop and the menu bar item.
//!
//! Memory rules as on Windows: screens of the same size share one rendered frame, and when no
//! screen can be seen the render targets are freed and the timer slows down to one tick a second.

mod menu;

use crate::config::{self, Config, PowerRule, Settings};
use crate::gpu::{Gpu, Target};
use crate::render::{Post, Renderer};
use crate::world::World;
use crate::{library, log, scene, SCENE_VIGNETTE};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWindowOcclusionState, NSWindowStyleMask};
use objc2_foundation::{NSProcessInfo, NSRect, NSTimer};
use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

/// Retina screens have four times the pixels; a wallpaper behind other windows does not need them
/// and every target would cost four times the memory.
const RETINA_SCALE: f64 = 0.6;
/// Seconds between two checks for changed screens and power state.
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

struct Screen {
    window: Retained<NSWindow>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    frame: NSRect,
    scale: f64,
}

impl Screen {
    fn hidden(&self) -> bool {
        !self.window.occlusionState().contains(NSWindowOcclusionState::Visible)
    }
}

fn create_screen(gpu: &Gpu, screen: &NSScreen, quality: f32, mtm: MainThreadMarker) -> Result<Screen, String> {
    let (frame, scale) = (screen.frame(), screen.backingScaleFactor());
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer_screen(NSWindow::alloc(mtm), frame, NSWindowStyleMask::Borderless, NSBackingStoreType::Buffered, false, Some(screen))
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
    window.orderFrontRegardless();
    log(&format!("{}x{} points at scale {scale}: drawing {}x{}", frame.size.width, frame.size.height, config.width, config.height));
    Ok(Screen { window, surface, config, frame, scale })
}

struct Engine {
    gpu: Gpu,
    post: Post,
    renderer: Renderer,
    world: World,
    cfg: Config,
    settings: Settings,
    screens: Vec<Screen>,
    targets: Vec<Target>,
    last: Instant,
    next_check: Instant,
    /// nothing is drawn right now and the targets are gone
    idle: bool,
    rule: PowerRule,
    mtm: MainThreadMarker,
}

impl Engine {
    fn build_screens(&mut self) {
        self.screens.clear();
        self.targets.clear();
        for screen in NSScreen::screens(self.mtm).iter() {
            match create_screen(&self.gpu, &screen, self.cfg.quality, self.mtm) {
                Ok(s) => self.screens.push(s),
                Err(e) => log(&format!("a screen could not be set up: {e}")),
            }
        }
    }

    fn screens_changed(&self) -> bool {
        let now = NSScreen::screens(self.mtm);
        now.len() != self.screens.len() || now.iter().zip(&self.screens).any(|(n, s)| n.frame() != s.frame || n.backingScaleFactor() != s.scale)
    }

    fn check(&mut self) {
        if self.screens_changed() {
            log("the screens changed");
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
        if now >= self.next_check {
            self.next_check = now + CHECK_EVERY;
            self.check();
        }
        let wanted = self.wanted();
        if !wanted.contains(&true) {
            if !self.idle {
                self.idle = true;
                self.targets.clear();
                self.renderer.release();
            }
            self.last = now;
            return self.interval() != speed;
        }
        self.idle = false;
        let dt = (now - self.last).as_secs_f32().min(0.25);
        self.last = now;
        self.world.update(dt, &self.settings);
        self.renderer.prepare(&self.gpu, &mut self.world);

        for (i, s) in self.screens.iter().enumerate().filter(|(i, _)| wanted[*i]) {
            let (w, h) = (s.config.width, s.config.height);
            let at = match self.targets.iter().position(|t| t.w == w && t.h == h) {
                Some(at) => at,
                None => {
                    self.targets.push(Target::new(&self.gpu, w, h));
                    self.targets.len() - 1
                }
            };
            // the first screen of a size renders, the others show the same frame
            if !self.screens[..i].iter().enumerate().any(|(j, o)| wanted[j] && o.config.width == w && o.config.height == h) {
                let view = self.world.view(w as f32 / h as f32, false);
                self.renderer.render(&self.gpu, &self.world, &view, &self.targets[at]);
            }
            let frame = match s.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                _ => {
                    s.surface.configure(&self.gpu.device, &s.config);
                    continue;
                }
            };
            let view = frame.texture.create_view(&Default::default());
            self.post.draw(&self.gpu, &self.targets[at], &view, self.cfg.fxaa, SCENE_VIGNETTE);
            self.gpu.queue.present(frame);
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
        e.cfg.paused = !e.cfg.paused;
        e.cfg.save();
        e.next_check = Instant::now();
        e.cfg.paused
    })
}

fn start(cfg: Config, settings: Settings, mtm: MainThreadMarker) -> Result<f64, String> {
    let gpu = Gpu::new()?;
    log(&format!("graphics: {}", gpu.name()));
    let post = Post::new(&gpu, wgpu::TextureFormat::Bgra8Unorm);
    let scene = scene::parse(library::PAGODA_SCENE)?;
    let renderer = Renderer::new(&gpu, &scene, scene.manifest.shadow.size);
    let world = World::new(scene.manifest);
    let now = Instant::now();
    let mut engine = Engine { gpu, post, renderer, world, cfg, settings, screens: Vec::new(), targets: Vec::new(), last: now, next_check: now + CHECK_EVERY, idle: false, rule: PowerRule::Keep, mtm };
    engine.build_screens();
    engine.check();
    let interval = engine.interval();
    ENGINE.with_borrow_mut(|e| *e = Some(engine));
    Ok(interval)
}

/// Only one engine per user: a second window stack on top of the first helps nobody.
fn single_instance() -> bool {
    use std::os::fd::AsRawFd;
    let _ = std::fs::create_dir_all(config::dir());
    let Ok(file) = std::fs::File::create(config::dir().join("kumo.lock")) else { return true };
    let free = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    std::mem::forget(file);
    free
}

pub fn run(cfg: Config, settings: Settings) {
    if !single_instance() {
        return log("already running");
    }
    let mtm = MainThreadMarker::new().expect("the engine runs on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // a menu bar item, but no Dock icon and no menu bar of its own
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let paused = cfg.paused;
    match start(cfg, settings, mtm) {
        Ok(interval) => {
            schedule(interval);
            menu::install(paused, toggle_pause, mtm);
            app.run();
        }
        Err(e) => log(&format!("no graphics device: {e}")),
    }
}

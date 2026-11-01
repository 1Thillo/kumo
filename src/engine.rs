//! The running wallpaper: one window + swap chain per monitor, one player per wallpaper that is
//! actually on screen, and the frame loop.
//!
//! Memory rules: a wallpaper that no screen shows is unloaded immediately; screens with the same
//! wallpaper and aspect ratio share one rendered frame; a still image is drawn once and its
//! texture freed, because the screen keeps showing it; when nothing needs drawing the render
//! targets are freed and the loop sleeps.

use crate::config::{self, Config, PowerRule, TimeMode};
use crate::library::{self, Kind};
use crate::render::{fit_uv, Post, Renderer};
use crate::world::World;
use crate::{desktop, gfx, log, scene, tray, watch, Ev};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView;
use windows::Win32::System::ProcessStatus::{K32EmptyWorkingSet, K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Strength of the soft vignette drawn over 3D scenes (matches the browser preview).
const SCENE_VIGNETTE: f32 = 0.28;

/// A video that fails this many frames in a row is given up on.
const MAX_FAILURES: u32 = 30;

enum Content {
    Scene { renderer: Renderer, world: World },
    /// `max` is the screen size the picture was scaled for, `srv` only exists while drawing
    Image { path: std::path::PathBuf, srv: Option<ID3D11ShaderResourceView>, w: u32, h: u32, max: (u32, u32) },
    Video { video: Box<crate::video::Video> },
}

pub struct Player {
    id: String,
    content: Content,
    failures: u32,
}

impl Player {
    fn animated(&self) -> bool {
        !matches!(self.content, Content::Image { .. })
    }
}

pub struct Output {
    pub container: HWND,
    pub hwnd: HWND,
    pub swap: gfx::Swap,
    pub mon: desktop::Monitor,
    pub covered: bool,
    pub wallpaper: String,
    /// a still picture has been presented and stays on screen by itself
    drawn: bool,
}

#[derive(Default)]
struct Stats {
    frames: u32,
    busy: Duration,
    since: Option<Instant>,
    every: Duration,
    proc_cpu: f64,
}

pub struct App {
    pub gpu: gfx::Gpu,
    post: Post,
    /// Media Foundation, started with the first video
    mf: Option<crate::video::Mf>,
    players: Vec<Player>,
    pub cfg: Config,
    /// scene settings given on the command line (`--hour 19` ...), applied on top of the config
    pub overrides: Vec<(String, String)>,
    pub outputs: Vec<Output>,
    targets: Vec<(gfx::Target, bool)>,
    pub instance: HINSTANCE,
    pub msg: HWND,
    pub tray: Option<tray::Tray>,
    pub windowed: Option<(u32, u32)>,
    locked: bool,
    remote: bool,
    display_off: bool,
    saver: bool,
    battery: bool,
    released: bool,
    fresh: bool,
    pub attach_denied: bool,
    /// wallpapers that could not be shown, with the reason (the panel reads them from status.json)
    failed: Vec<(String, String)>,
    /// outputs are rebuilt at this moment (docking sends several display changes in a row)
    rebuild_at: Option<Instant>,
    /// the desktop's window order is checked at this moment, and `order_left` more times after
    /// it: Windows takes a few seconds to set a new wallpaper
    order_at: Option<Instant>,
    order_left: u32,
    pub desktop: Option<desktop::Desktop>,
    stats: Stats,
}

impl App {
    pub fn new(gpu: gfx::Gpu, cfg: Config, instance: HINSTANCE, msg: HWND, windowed: Option<(u32, u32)>, stats_every: Duration) -> windows::core::Result<App> {
        Ok(App {
            post: Post::new(&gpu)?,
            mf: None,
            gpu,
            players: Vec::new(),
            cfg,
            overrides: Vec::new(),
            outputs: Vec::new(),
            targets: Vec::new(),
            instance,
            msg,
            tray: None,
            windowed,
            locked: false,
            remote: unsafe { GetSystemMetrics(SM_REMOTESESSION) } != 0,
            display_off: false,
            saver: false,
            battery: false,
            released: false,
            fresh: true,
            attach_denied: false,
            failed: Vec::new(),
            rebuild_at: None,
            order_at: None,
            order_left: 0,
            desktop: None,
            stats: Stats { every: stats_every, ..Default::default() },
        })
    }

    // ------------------------------------------------------------ windows and players

    pub fn destroy_outputs(&mut self) {
        for p in &mut self.players {
            if let Content::Video { video } = &mut p.content {
                video.outputs_changed();
            }
        }
        for o in self.outputs.drain(..) {
            drop(o.swap);
            unsafe {
                let _ = DestroyWindow(o.hwnd);
                if o.container != o.hwnd {
                    let _ = DestroyWindow(o.container);
                }
            }
        }
        self.targets.clear();
    }

    fn device_lost(&self) -> bool {
        unsafe { self.gpu.dev.GetDeviceRemovedReason() }.is_err()
    }

    /// A driver update or GPU reset took the device with it: start over on a new one.
    fn recover_device(&mut self) {
        log(&format!("kumo: the graphics device was lost: {}", unsafe { self.gpu.dev.GetDeviceRemovedReason() }.err().map(|e| e.message()).unwrap_or_default()));
        self.destroy_outputs();
        self.players.clear();
        self.mf = None;
        match gfx::Gpu::new().and_then(|gpu| Ok((Post::new(&gpu)?, gpu))) {
            Ok((post, gpu)) => {
                (self.post, self.gpu) = (post, gpu);
                self.build_outputs();
            }
            Err(e) => log(&format!("kumo: no graphics device yet: {}", e.message())),
        }
    }

    /// What a screen shows: its wallpaper, or the built-in scene if that one failed.
    fn shown_on(&self, monitor_id: &str) -> String {
        let w = if self.windowed.is_some() { &self.cfg.wallpaper } else { self.cfg.wallpaper_for(monitor_id) };
        if self.failed.iter().any(|(id, _)| id == w) { config::BUILTIN_SCENE.into() } else { w.to_string() }
    }

    pub fn build_outputs(&mut self) {
        if self.device_lost() {
            return self.recover_device();
        }
        self.destroy_outputs();
        self.fresh = true;
        if let Some((w, h)) = self.windowed {
            if let Some(hwnd) = desktop::create_test_window(w as i32, h as i32, self.instance) {
                if let Ok(swap) = gfx::Swap::new(&self.gpu, hwnd, w, h) {
                    let mon = desktop::Monitor { rect: RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }, id: "window".into(), name: "Window".into(), primary: true };
                    let wallpaper = self.shown_on(&mon.id);
                    self.outputs.push(Output { container: hwnd, hwnd, swap, mon, covered: false, wallpaper, drawn: false });
                }
            }
            self.sync_players();
            return;
        }
        let Some(d) = desktop::find() else {
            log("kumo: desktop window not found (is Explorer running?)");
            return;
        };
        self.desktop = Some(d);
        for mon in desktop::monitors() {
            if self.cfg.off.contains(&mon.id) {
                continue;
            }
            let (container, hwnd) = match desktop::create(&d, &mon, self.instance) {
                Ok(w) => w,
                Err(e) => {
                    log(&format!("kumo: could not create a wallpaper window for {:?}", mon.rect));
                    self.attach_denied |= e.code() == E_ACCESSDENIED;
                    continue;
                }
            };
            // the swap chain normally lives on the inner window; the container is the fallback
            let made = gfx::Swap::new(&self.gpu, hwnd, mon.w(), mon.h()).map(|s| (s, hwnd)).or_else(|e| {
                log(&format!("kumo: swap chain on the view window failed ({}), trying the container", e.message()));
                gfx::Swap::new(&self.gpu, container, mon.w(), mon.h()).map(|s| (s, container))
            });
            match made {
                Ok((swap, hwnd)) => {
                    log(&format!("kumo: {} ({}x{}) swap chain ({} model)", mon.name, swap.w, swap.h, if swap.flip { "flip" } else { "blt" }));
                    let wallpaper = self.shown_on(&mon.id);
                    self.outputs.push(Output { container, hwnd, swap, mon, covered: false, wallpaper, drawn: false });
                }
                Err(e) => unsafe {
                    log(&format!("kumo: swap chain for {:?} failed: {}", mon.rect, e.message()));
                    let _ = DestroyWindow(container);
                },
            }
        }
        self.sync_players();
        self.refresh_cover();
    }

    /// Loads the wallpapers that are on screen and unloads the rest.
    fn sync_players(&mut self) {
        let wanted: Vec<String> = self.outputs.iter().map(|o| o.wallpaper.clone()).collect();
        let size_for = |id: &str, outputs: &[Output]| outputs.iter().filter(|o| o.wallpaper == id).fold((1, 1), |(w, h), o| (w.max(o.swap.w), h.max(o.swap.h)));
        let outputs = &self.outputs;
        self.players.retain(|p| {
            // a picture scaled for a smaller screen than the one it is on now would look soft
            let sharp = match &p.content {
                Content::Image { max, .. } => {
                    let need = size_for(&p.id, outputs);
                    max.0 >= need.0 && max.1 >= need.1
                }
                _ => true,
            };
            wanted.contains(&p.id) && sharp
        });
        if !self.players.iter().any(|p| matches!(p.content, Content::Video { .. })) {
            self.mf = None;
        }
        for id in wanted {
            if self.players.iter().any(|p| p.id == id) {
                continue;
            }
            let (mw, mh) = size_for(&id, &self.outputs);
            match library::get(&id).ok_or_else(|| "not in the library".to_string()).and_then(|item| load(&self.gpu, &mut self.mf, &item, mw, mh)) {
                Ok(p) => self.players.push(p),
                Err(e) if id == config::BUILTIN_SCENE => log(&format!("kumo: the built-in scene could not be loaded: {e}")),
                Err(e) => return self.give_up(&id, &e),
            }
        }
    }

    /// A wallpaper that cannot be shown: its screens go to the built-in scene, and the panel gets
    /// to know why.
    fn give_up(&mut self, id: &str, why: &str) {
        log(&format!("kumo: wallpaper '{id}' cannot be shown: {why} - showing the built-in scene"));
        self.failed.push((id.to_string(), why.to_string()));
        self.write_status();
        for o in self.outputs.iter_mut().filter(|o| o.wallpaper == id) {
            o.wallpaper = config::BUILTIN_SCENE.into();
            o.drawn = false;
        }
        self.sync_players();
    }

    fn write_status(&self) {
        if let Ok(text) = serde_json::to_string(&self.failed) {
            let _ = std::fs::write(config::dir().join("status.json"), text);
        }
    }

    /// Re-reads config.json after the control panel changed it.
    pub fn reload(&mut self) {
        let (quality, off) = (self.cfg.quality, self.cfg.off.clone());
        self.cfg = Config::load();
        for (k, v) in &self.overrides {
            self.cfg.set(k, v);
        }
        if self.cfg.off != off {
            self.build_outputs();
            if let Some(d) = &self.desktop {
                desktop::restore_wallpaper(d);
            }
        }
        // a failed wallpaper gets another try once the user has picked something else
        let requested: Vec<String> = self.outputs.iter().map(|o| if self.windowed.is_some() { self.cfg.wallpaper.clone() } else { self.cfg.wallpaper_for(&o.mon.id).to_string() }).collect();
        let before = self.failed.len();
        self.failed.retain(|(id, _)| requested.contains(id));
        if self.failed.len() != before {
            self.write_status();
        }
        for i in 0..self.outputs.len() {
            self.outputs[i].wallpaper = self.shown_on(&self.outputs[i].mon.id.clone());
            // media settings (fit) may have changed too: redraw still pictures
            self.outputs[i].drawn = false;
        }
        if (quality - self.cfg.quality).abs() > 0.001 {
            self.targets.clear();
        }
        self.sync_players();
        self.update_tooltip();
    }

    /// Re-checks which monitors are covered by maximized / full-screen windows.
    pub fn refresh_cover(&mut self) {
        if self.windowed.is_some() {
            return;
        }
        let mons: Vec<_> = self.outputs.iter().map(|o| o.mon.clone()).collect();
        let covered = if self.cfg.pause_when_covered { watch::covered(&mons) } else { vec![false; mons.len()] };
        for (o, c) in self.outputs.iter_mut().zip(covered) {
            o.covered = c;
        }
    }

    fn power_rule(&self) -> PowerRule {
        let mut r = PowerRule::Keep;
        for (active, rule) in [(self.battery, self.cfg.on_battery), (self.saver, self.cfg.energy_saver)] {
            if active && (rule == PowerRule::Pause || r == PowerRule::Keep) {
                r = rule;
            }
        }
        r
    }

    fn needs_frame(&self, o: &Output) -> bool {
        (self.fresh || !o.covered) && (!o.drawn || self.players.iter().any(|p| p.id == o.wallpaper && p.animated()))
    }

    /// Paused by the user or a power rule, or nobody can see the screen.
    pub fn held(&self) -> bool {
        self.cfg.paused || self.locked || self.remote || self.display_off || self.power_rule() == PowerRule::Pause
    }

    /// True while there are screens that should show a wallpaper but none does yet.
    pub fn wants_outputs(&self) -> bool {
        self.outputs.is_empty() && self.windowed.is_none() && desktop::monitors().iter().any(|m| !self.cfg.off.contains(&m.id))
    }

    /// A screen that was never drawn is black: it gets one frame even during a pause.
    fn owed_frame(&self, o: &Output) -> bool {
        !o.drawn && (self.fresh || !o.covered) && !self.locked && !self.remote && !self.display_off
    }

    fn wants_frame(&self, o: &Output) -> bool {
        if self.held() { self.owed_frame(o) } else { self.needs_frame(o) }
    }

    pub fn paused(&self) -> bool {
        !self.outputs.iter().any(|o| self.wants_frame(o))
    }

    /// How long the paused loop may sleep: window events and everything else that can end a pause
    /// wake it anyway, only uncovering by a maximized window has to be noticed by looking.
    pub fn idle_timeout(&self) -> u32 {
        let looking = !self.held() && self.players.iter().any(Player::animated);
        if self.rebuild_at.is_some() || self.order_at.is_some() {
            300
        } else if self.wants_outputs() || looking {
            2000
        } else {
            60_000
        }
    }

    /// Rebuilds what vanished behind our back: the GPU, or windows whose parent died.
    pub fn check_health(&mut self) {
        let dead = self.outputs.iter().any(|o| !unsafe { IsWindow(o.hwnd) }.as_bool());
        if dead || self.device_lost() {
            self.build_outputs();
        }
        if let Some(d) = &self.desktop {
            desktop::keep_order(d);
        }
    }

    pub fn rebuild_if_due(&mut self, now: Instant) {
        if self.rebuild_at.is_some_and(|t| now >= t) {
            self.rebuild_at = None;
            self.build_outputs();
        }
        if self.order_at.is_some_and(|t| now >= t) {
            self.order_at = (self.order_left > 0).then(|| now + Duration::from_secs(1));
            self.order_left = self.order_left.saturating_sub(1);
            if let Some(d) = &self.desktop {
                desktop::keep_order(d);
            }
        }
    }

    /// Gives memory back while nothing needs drawing: render targets, shadow maps, driver caches.
    pub fn release(&mut self) {
        self.targets.clear();
        for p in &mut self.players {
            match &mut p.content {
                Content::Scene { renderer, .. } => renderer.release(),
                Content::Image { srv, .. } => *srv = None,
                Content::Video { .. } => {}
            }
        }
        self.gpu.trim();
        self.released = true;
    }

    /// After a longer pause the video decoders go too; they reopen at the same position.
    pub fn release_media(&mut self) {
        for p in &mut self.players {
            if let Content::Video { video } = &mut p.content {
                video.release();
            }
        }
        self.gpu.trim();
        // nothing moves for now: what is left goes back to Windows until it is needed again
        unsafe {
            let _ = K32EmptyWorkingSet(GetCurrentProcess());
        }
    }

    pub fn is_released(&self) -> bool {
        self.released
    }

    fn target(&mut self, w: u32, h: u32) -> Option<usize> {
        if let Some(i) = self.targets.iter().position(|(t, _)| t.w == w && t.h == h) {
            self.targets[i].1 = true;
            return Some(i);
        }
        let t = gfx::Target::new(&self.gpu, w, h).ok()?;
        self.targets.push((t, true));
        Some(self.targets.len() - 1)
    }

    pub fn interval(&self) -> Duration {
        let fps = if self.power_rule() == PowerRule::Half { (self.cfg.fps / 2).max(10) } else { self.cfg.fps };
        Duration::from_secs_f64(1.0 / fps as f64)
    }

    // ------------------------------------------------------------ frame

    fn scene_settings(&self, id: &str) -> config::Settings {
        let mut s = self.cfg.scene(id);
        for (k, v) in &self.overrides {
            s.set(k, v);
        }
        s
    }

    fn mouse_in(&self, o: &Output) -> (f32, f32) {
        let mut p = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut p);
        }
        let r = o.mon.rect;
        let x = ((p.x - r.left) as f32 / o.mon.w() as f32 * 2.0 - 1.0).clamp(-1.0, 1.0);
        let y = ((p.y - r.top) as f32 / o.mon.h() as f32 * 2.0 - 1.0).clamp(-1.0, 1.0);
        (x, y)
    }

    pub fn frame(&mut self, dt: f32) {
        let started = Instant::now();
        self.released = false;
        for t in &mut self.targets {
            t.1 = false;
        }
        let todo: Vec<usize> = (0..self.outputs.len()).filter(|&i| self.wants_frame(&self.outputs[i])).collect();
        self.fresh = false;
        let mut presented: Vec<usize> = Vec::new();
        let mut broken: Vec<(String, String)> = Vec::new();
        let mut players = std::mem::take(&mut self.players);
        for p in &mut players {
            let mine: Vec<usize> = todo.iter().copied().filter(|&i| self.outputs[i].wallpaper == p.id).collect();
            if mine.is_empty() {
                continue;
            }
            let mut drawn = mine.clone();
            match &mut p.content {
                Content::Scene { renderer, world } => {
                    let s = self.scene_settings(&p.id);
                    if s.parallax {
                        let (x, y) = self.mouse_in(&self.outputs[mine[0]]);
                        world.set_mouse(x, y, dt);
                    }
                    world.update(dt, &s);
                    if renderer.prepare(&self.gpu, world).is_err() {
                        continue;
                    }
                    // screens with the same aspect ratio share one rendered frame
                    let mut done = vec![false; mine.len()];
                    for a in 0..mine.len() {
                        if done[a] {
                            continue;
                        }
                        let aspect = |i: usize| self.outputs[i].swap.w as f32 / self.outputs[i].swap.h as f32;
                        let group: Vec<usize> = (a..mine.len()).filter(|&b| !done[b] && (aspect(mine[b]) - aspect(mine[a])).abs() < 0.01).collect();
                        let gw = group.iter().map(|&b| self.outputs[mine[b]].swap.w).max().unwrap_or(1);
                        let gh = group.iter().map(|&b| self.outputs[mine[b]].swap.h).max().unwrap_or(1);
                        let rw = ((gw as f32 * self.cfg.quality).round() as u32).max(16);
                        let rh = ((gh as f32 * self.cfg.quality).round() as u32).max(16);
                        let Some(ti) = self.target(rw, rh) else {
                            drawn.retain(|i| !group.iter().any(|&b| mine[b] == *i));
                            group.iter().for_each(|&b| done[b] = true);
                            continue;
                        };
                        let view = world.view(gw as f32 / gh as f32, s.parallax);
                        renderer.render(&self.gpu, world, &view, &self.targets[ti].0);
                        for &b in &group {
                            let o = &self.outputs[mine[b]];
                            let t = &self.targets[ti].0;
                            self.post.draw(&self.gpu, &t.srv, (t.w, t.h), &o.swap.rtv, (o.swap.w, o.swap.h), [1.0, 1.0, 0.0, 0.0], self.cfg.fxaa, SCENE_VIGNETTE);
                            done[b] = true;
                        }
                    }
                }
                Content::Image { path, srv, w, h, max } => {
                    if srv.is_none() {
                        match picture(&self.gpu, path, *max) {
                            Ok((s, pw, ph)) => (*srv, *w, *h) = (Some(s), pw, ph),
                            Err(e) => {
                                broken.push((p.id.clone(), e));
                                continue;
                            }
                        }
                    }
                    let fit = self.cfg.media(&p.id).fit;
                    for &i in &mine {
                        let o = &self.outputs[i];
                        let uv = fit_uv(fit, *w, *h, o.swap.w, o.swap.h);
                        self.post.draw(&self.gpu, srv.as_ref().unwrap(), (*w, *h), &o.swap.rtv, (o.swap.w, o.swap.h), uv, false, 0.0);
                    }
                }
                Content::Video { video } => {
                    let m = self.cfg.media(&p.id);
                    let Some(mf) = &self.mf else { continue };
                    let redraw = mine.iter().any(|&i| !self.outputs[i].drawn);
                    let result = video.advance(mf, &self.gpu, dt, m.speed).and_then(|changed| {
                        if changed || redraw {
                            mine.iter().try_for_each(|&i| video.draw(&self.outputs[i].swap, m.fit))?;
                        }
                        Ok(changed || redraw)
                    });
                    match result {
                        Ok(true) => p.failures = 0,
                        // no new frame yet: the old one stays on screen, nothing to present
                        Ok(false) => continue,
                        Err(e) => {
                            p.failures += 1;
                            if p.failures >= MAX_FAILURES {
                                broken.push((p.id.clone(), e.message()));
                            }
                            continue;
                        }
                    }
                }
            }
            for &i in &drawn {
                self.outputs[i].drawn = true;
                presented.push(i);
            }
        }
        self.players = players;
        for &i in &presented {
            self.outputs[i].swap.present();
        }
        // a picture stays on screen by itself; its texture is only needed to draw it again
        if self.outputs.iter().all(|o| o.drawn) {
            for p in &mut self.players {
                if let Content::Image { srv, .. } = &mut p.content {
                    *srv = None;
                }
            }
        }
        self.targets.retain(|t| t.1);
        self.count(started, todo.len());
        for (id, why) in broken {
            self.give_up(&id, &why);
        }
    }

    fn count(&mut self, started: Instant, drawn: usize) {
        let st = &mut self.stats;
        st.frames += 1;
        st.busy += started.elapsed();
        let since = *st.since.get_or_insert_with(Instant::now);
        if since.elapsed() < st.every {
            return;
        }
        let secs = since.elapsed().as_secs_f64();
        let (ws, private) = memory();
        let cpu = process_cpu_seconds();
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        log(&format!(
            "{:5.1} fps | CPU {:.1} % of the PC | frame {:.2} ms incl. waiting for the GPU | working set {:.0} MB | memory (private working set) {:.0} MB | {} of {} screens drawn | {} wallpaper(s) loaded",
            st.frames as f64 / secs,
            (cpu - st.proc_cpu) / secs / cores * 100.0,
            st.busy.as_secs_f64() * 1000.0 / st.frames.max(1) as f64,
            ws, private, drawn, self.outputs.len(), self.players.len()
        ));
        *st = Stats { since: Some(Instant::now()), every: st.every, proc_cpu: cpu, ..Default::default() };
    }

    // ------------------------------------------------------------ events and tray

    pub fn update_tooltip(&mut self) {
        let tip = tray::tooltip(self.cfg.paused);
        if let Some(t) = &mut self.tray {
            t.set_tooltip(&tip);
        }
    }

    /// Returns false when Kumo should quit.
    pub fn handle(&mut self, ev: Ev) -> bool {
        match ev {
            Ev::Tray => {
                let c = tray::show_menu(self.msg, &self.cfg);
                return self.command(c);
            }
            Ev::Displays => self.rebuild_at = Some(Instant::now() + Duration::from_millis(300)),
            Ev::WallpaperChanged => {
                self.order_at = Some(Instant::now() + Duration::from_millis(300));
                self.order_left = 5;
            }
            Ev::Taskbar => {
                if let Some(t) = &self.tray {
                    t.add();
                }
                self.build_outputs();
            }
            Ev::Reload => self.reload(),
            Ev::OpenPanel => crate::open_panel(),
            Ev::Restart => {
                // the installer replaced our exe: hand over to the new one
                crate::release_single_instance();
                let exe = windows::core::HSTRING::from(crate::install::installed_exe().as_os_str());
                unsafe {
                    windows::Win32::UI::Shell::ShellExecuteW(None, windows::core::w!("open"), &exe, windows::core::PCWSTR::null(), windows::core::PCWSTR::null(), SW_SHOWNORMAL);
                }
                return false;
            }
            Ev::Lock(l) => self.locked = l,
            Ev::Remote(r) => self.remote = r,
            Ev::DisplayOn(on) => self.display_off = !on,
            Ev::Saver(on) => self.saver = on,
            Ev::Battery(on) => self.battery = on,
            Ev::Quit => return false,
        }
        true
    }

    fn command(&mut self, c: u32) -> bool {
        use tray::cmd;
        match c {
            cmd::OPEN => {
                crate::open_panel();
                return true;
            }
            cmd::PAUSE => self.cfg.paused = !self.cfg.paused,
            cmd::QUIT => return false,
            c if (cmd::TIME..cmd::TIME + 6).contains(&c) => {
                let ids: Vec<String> = self.players.iter().filter(|p| p.animated()).map(|p| p.id.clone()).collect();
                for id in ids {
                    let s = self.cfg.scene_mut(&id);
                    match c - cmd::TIME {
                        0 => s.time = TimeMode::Real,
                        5 => s.time = TimeMode::Lapse,
                        i => {
                            s.time = TimeMode::Fixed;
                            s.hour = tray::FIXED_HOURS[i as usize - 1];
                        }
                    }
                }
                if c - cmd::TIME == 5 {
                    for p in &mut self.players {
                        if let Content::Scene { world, .. } = &mut p.content {
                            world.reset_clock();
                        }
                    }
                }
            }
            c if (cmd::FPS..cmd::FPS + 3).contains(&c) => self.cfg.fps = tray::FPS_STEPS[(c - cmd::FPS) as usize],
            _ => return true,
        }
        self.cfg.save();
        self.update_tooltip();
        true
    }
}

/// Loads a wallpaper for screens up to `max_w` x `max_h`.
fn load(gpu: &gfx::Gpu, mf: &mut Option<crate::video::Mf>, item: &library::Item, max_w: u32, max_h: u32) -> Result<Player, String> {
    let content = match item.kind {
        Kind::Scene => {
            let bytes = item.scene_bytes().map_err(|e| e.to_string())?;
            let scene = scene::parse(&bytes)?;
            let renderer = Renderer::new(gpu, &scene, scene.manifest.shadow.size).map_err(|e| e.message())?;
            Content::Scene { renderer, world: World::new(scene.manifest) }
        }
        Kind::Image => {
            let (srv, w, h) = picture(gpu, &item.path(), (max_w, max_h))?;
            Content::Image { path: item.path(), srv: Some(srv), w, h, max: (max_w, max_h) }
        }
        Kind::Video => {
            if mf.is_none() {
                *mf = Some(crate::video::Mf::new(gpu).map_err(|e| e.message())?);
            }
            let v = crate::video::Video::new(mf.as_ref().unwrap(), gpu, &item.path()).map_err(|e| e.message())?;
            Content::Video { video: Box::new(v) }
        }
    };
    log(&format!("kumo: loaded '{}' ({:?})", item.title, item.kind));
    Ok(Player { id: item.id.clone(), content, failures: 0 })
}

fn picture(gpu: &gfx::Gpu, path: &std::path::Path, (max_w, max_h): (u32, u32)) -> Result<(ID3D11ShaderResourceView, u32, u32), String> {
    let (px, w, h) = crate::media::load_bgra(path, max_w, max_h).map_err(|e| e.message())?;
    Ok((gfx::texture_bgra(gpu, &px, w, h).map_err(|e| e.message())?, w, h))
}

/// CPU time (user + kernel) this process has used so far, in seconds.
fn process_cpu_seconds() -> f64 {
    let (mut c, mut e, mut k, mut u) = Default::default();
    unsafe {
        let _ = GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u);
    }
    let secs = |t: FILETIME| ((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) as f64 / 1e7;
    secs(k) + secs(u)
}

/// PROCESS_MEMORY_COUNTERS_EX2 (Windows 10 1809+), not in windows 0.58.
#[repr(C)]
#[derive(Default)]
struct MemoryCounters {
    cb: u32,
    page_faults: u32,
    peak_working_set: usize,
    working_set: usize,
    quota: [usize; 4],
    pagefile: usize,
    peak_pagefile: usize,
    private_usage: usize,
    private_working_set: usize,
    shared_commit: u64,
}

/// (working set, private working set) of a process in MB. The private working set is what
/// Task Manager shows as "Memory".
pub fn process_memory(process: HANDLE) -> Option<(f64, f64)> {
    let mut m = MemoryCounters { cb: std::mem::size_of::<MemoryCounters>() as u32, ..Default::default() };
    let ok = unsafe { K32GetProcessMemoryInfo(process, &mut m as *mut _ as *mut PROCESS_MEMORY_COUNTERS, m.cb).as_bool() };
    if !ok || m.private_working_set == 0 {
        // older Windows: fall back to the classic counters
        let mut pmc = PROCESS_MEMORY_COUNTERS_EX { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
        unsafe { K32GetProcessMemoryInfo(process, &mut pmc as *mut _ as *mut PROCESS_MEMORY_COUNTERS, pmc.cb).ok().ok()? };
        return Some((pmc.WorkingSetSize as f64 / 1048576.0, pmc.PrivateUsage as f64 / 1048576.0));
    }
    Some((m.working_set as f64 / 1048576.0, m.private_working_set as f64 / 1048576.0))
}

pub fn memory() -> (f64, f64) {
    process_memory(unsafe { GetCurrentProcess() }).unwrap_or_default()
}

// ---------------------------------------------------------------- main loop

fn wait(timer: HANDLE, until: Instant) {
    unsafe {
        let left = until.saturating_duration_since(Instant::now());
        let due = -((left.as_nanos() / 100) as i64).max(1);
        let _ = SetWaitableTimer(timer, &due, 0, None, None, false);
        MsgWaitForMultipleObjectsEx(Some(&[timer]), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
}

pub fn run(app: &mut App) {
    let timer = unsafe { CreateWaitableTimerExW(None, windows::core::PCWSTR::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, 0x1F0003) }
        .or_else(|_| unsafe { CreateWaitableTimerExW(None, windows::core::PCWSTR::null(), 0, 0x1F0003) })
        .unwrap_or_default();
    let mut last = Instant::now();
    let mut next = last;
    let mut last_watch = last - Duration::from_secs(5);
    let mut last_retry = last;
    let mut paused_at = last;
    let mut media_released = false;
    loop {
        let mut msg = MSG::default();
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        for ev in crate::take_events() {
            if !app.handle(ev) {
                return;
            }
        }
        let now = Instant::now();
        app.rebuild_if_due(now);
        let since = now - last_watch;
        if since >= Duration::from_secs(2) || (since >= Duration::from_millis(100) && watch::take_dirty()) {
            app.check_health();
            if now - last_retry >= Duration::from_secs(5) && app.wants_outputs() {
                app.build_outputs(); // Explorer not ready yet (e.g. right after login)
                last_retry = now;
            }
            if !app.held() {
                app.refresh_cover();
                last_watch = now;
            }
        }
        if app.paused() {
            if !app.is_released() {
                app.release();
                paused_at = now;
                media_released = false;
            }
            if !media_released && now - paused_at > Duration::from_secs(30) {
                app.release_media();
                media_released = true;
            }
            // the step above must not sleep through its 30 seconds
            let until_release = if media_released { u32::MAX } else { (Duration::from_secs(31).saturating_sub(now - paused_at)).as_millis() as u32 };
            unsafe { MsgWaitForMultipleObjectsEx(None, app.idle_timeout().min(until_release), QS_ALLINPUT, MWMO_INPUTAVAILABLE) };
            last = Instant::now();
            next = last;
            continue;
        }
        if now < next {
            wait(timer, next);
            continue;
        }
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;
        next += app.interval();
        if next < now {
            next = now + app.interval();
        }
        app.frame(dt);
    }
}

//! Kumo: a tiny live wallpaper engine for Windows.
//!
//! One process, one Direct3D 11 device for all monitors, no browser. The same exe is also the
//! control panel (`--panel`, a separate process that only exists while it is open) and its own
//! installer. See engine.rs for the frame loop and its memory rules.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod config;
mod desktop;
mod engine;
mod env;
mod gfx;
mod install;
mod library;
mod media;
mod os;
mod panel;
mod render;
mod scene;
mod setup;
mod tray;
mod video;
mod watch;
mod world;

use config::Config;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use windows::core::{w, GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
use windows::Win32::System::Recovery::{RegisterApplicationRestart, RESTART_NO_CRASH, RESTART_NO_HANG, RESTART_NO_REBOOT};
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::Threading::*;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

static LOG: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);
static RECENT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Diagnostics go to the log file (`%APPDATA%\Kumo\kumo.log` or `--log`), else the console.
pub fn log(line: &str) {
    use std::io::Write;
    if let Ok(mut r) = RECENT.lock() {
        if r.len() >= 12 {
            r.remove(0);
        }
        r.push(line.to_string());
    }
    match LOG.lock().ok().as_deref_mut().and_then(|f| f.as_mut()) {
        Some(f) => {
            let _ = writeln!(f, "{line}");
        }
        None => eprintln!("{line}"),
    }
}

const GUID_CONSOLE_DISPLAY_STATE: GUID = GUID::from_u128(0x6fe69556_704a_47a0_8f24_c28d936fda47);
const GUID_POWER_SAVING_STATUS: GUID = GUID::from_u128(0xe00958c0_c213_4ace_ac77_fecced2eeea5);
const GUID_ACDC_POWER_SOURCE: GUID = GUID::from_u128(0x5d3e9a59_e9d5_4b00_a6bd_ff34ff516548);
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const PBT_POWERSETTINGCHANGE: usize = 0x8013;

/// Sent to the engine's message window by the control panel after it saved config.json.
pub const WM_KUMO_RELOAD: u32 = WM_APP + 2;
/// Sent by a second start of kumo.exe: open the control panel.
pub const WM_KUMO_OPEN: u32 = WM_APP + 3;
/// Sent by the installer after an update replaced the exe: start the new one and quit.
pub const WM_KUMO_RESTART: u32 = WM_APP + 4;

// ---------------------------------------------------------------- window messages -> events

pub enum Ev {
    Tray,
    Displays,
    WallpaperChanged,
    Taskbar,
    Reload,
    OpenPanel,
    Restart,
    Lock(bool),
    Remote(bool),
    DisplayOn(bool),
    Saver(bool),
    Battery(bool),
    Quit,
}

thread_local! {
    static EVENTS: RefCell<Vec<Ev>> = const { RefCell::new(Vec::new()) };
}
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(u32::MAX);

fn push(e: Ev) {
    EVENTS.with(|q| q.borrow_mut().push(e));
}

pub fn take_events() -> Vec<Ev> {
    EVENTS.with(|q| q.borrow_mut().drain(..).collect())
}

extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => return LRESULT(1),
            WM_PAINT => {
                let _ = ValidateRect(h, None);
                return LRESULT(0);
            }
            tray::WM_TRAY => {
                let e = (lp.0 & 0xffff) as u32;
                if e == WM_LBUTTONUP {
                    push(Ev::OpenPanel);
                } else if e == WM_RBUTTONUP || e == WM_CONTEXTMENU {
                    push(Ev::Tray);
                }
                return LRESULT(0);
            }
            WM_KUMO_RELOAD => push(Ev::Reload),
            WM_KUMO_OPEN => push(Ev::OpenPanel),
            WM_KUMO_RESTART => push(Ev::Restart),
            WM_DISPLAYCHANGE => push(Ev::Displays),
            WM_SETTINGCHANGE if wp.0 == SPI_SETDESKWALLPAPER.0 as usize => push(Ev::WallpaperChanged),
            WM_WTSSESSION_CHANGE => match wp.0 {
                1 | 4 => push(Ev::Remote(false)),
                3 => push(Ev::Remote(true)),
                7 => push(Ev::Lock(true)),
                8 => push(Ev::Lock(false)),
                _ => {}
            },
            WM_POWERBROADCAST if wp.0 == PBT_POWERSETTINGCHANGE => {
                let s = &*(lp.0 as *const POWERBROADCAST_SETTING);
                if s.PowerSetting == GUID_CONSOLE_DISPLAY_STATE {
                    push(Ev::DisplayOn(s.Data[0] != 0));
                } else if s.PowerSetting == GUID_POWER_SAVING_STATUS {
                    push(Ev::Saver(s.Data[0] != 0));
                } else if s.PowerSetting == GUID_ACDC_POWER_SOURCE {
                    push(Ev::Battery(s.Data[0] == 1));
                }
                return LRESULT(1);
            }
            WM_CLOSE => {
                push(Ev::Quit);
                return LRESULT(0);
            }
            m if m == TASKBAR_CREATED.load(Ordering::Relaxed) => push(Ev::Taskbar),
            _ => {}
        }
        DefWindowProcW(h, msg, wp, lp)
    }
}

/// The running engine's hidden message window, if Kumo runs.
pub fn engine_window() -> Option<HWND> {
    unsafe { FindWindowW(desktop::CLASS, w!("Kumo")).ok().filter(|h| !h.is_invalid()) }
}

/// Tells the running engine to re-read config.json.
pub fn notify_engine() {
    if let Some(h) = engine_window() {
        unsafe {
            let _ = PostMessageW(h, WM_KUMO_RELOAD, WPARAM(0), LPARAM(0));
        }
    }
}

/// Starts the control panel as its own process (it frees all its memory when closed).
pub fn open_panel() {
    start_panel(w!("--panel"));
}

fn start_panel(args: PCWSTR) {
    if let Ok(exe) = std::env::current_exe() {
        let exe = HSTRING::from(exe.as_os_str());
        unsafe {
            ShellExecuteW(None, w!("open"), &exe, args, PCWSTR::null(), SW_SHOWNORMAL);
        }
    }
}

// ---------------------------------------------------------------- entry

struct Args {
    window: Option<(u32, u32)>,
    shot: Option<String>,
    size: (u32, u32),
    warmup: f32,
    stats: bool,
    scene: Option<String>,
    overrides: Vec<(String, String)>,
    mode: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args { window: None, shot: None, size: (1280, 720), warmup: 6.0, stats: false, scene: None, overrides: Vec::new(), mode: None };
    let mut it = std::env::args().skip(1);
    let size = |s: Option<String>| {
        s.and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))).unwrap_or((1280, 720))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--window" => a.window = Some(a.size),
            "--size" => {
                a.size = size(it.next());
                if a.window.is_some() {
                    a.window = Some(a.size);
                }
            }
            "--shot" => a.shot = it.next(),
            "--warmup" => a.warmup = it.next().and_then(|v| v.parse().ok()).unwrap_or(6.0),
            "--stats" => a.stats = true,
            "--log" => {
                if let Some(p) = it.next() {
                    *LOG.lock().unwrap() = std::fs::File::create(p).ok();
                }
            }
            "--scene" => a.scene = it.next(),
            "--panel" | "--setup" | "--install" | "--uninstall" => a.mode = Some(arg.clone()),
            "--hour" => {
                a.overrides.push(("time".into(), "fixed".into()));
                a.overrides.push(("hour".into(), it.next().unwrap_or_default()));
            }
            "--lapse" => {
                a.overrides.push(("time".into(), "lapse".into()));
                a.overrides.push(("lapse".into(), it.next().unwrap_or_default()));
            }
            s if s.starts_with("--") && s.contains('=') => {
                let (k, v) = s[2..].split_once('=').unwrap();
                a.overrides.push((k.into(), v.into()));
            }
            _ => {}
        }
    }
    a
}

pub fn fail(msg: &str) -> ! {
    let text = HSTRING::from(msg);
    unsafe {
        MessageBoxW(None, &text, w!("Kumo"), MB_ICONERROR);
    }
    std::process::exit(1);
}

fn write_bmp(path: &str, w: u32, h: u32, rgba: &[u8]) -> std::io::Result<()> {
    let row = (w * 3 + 3) & !3;
    let mut out = Vec::with_capacity((54 + row * h) as usize);
    let le = |v: u32| v.to_le_bytes();
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&le(54 + row * h));
    out.extend_from_slice(&le(0));
    out.extend_from_slice(&le(54));
    out.extend_from_slice(&le(40));
    out.extend_from_slice(&le(w));
    out.extend_from_slice(&le(h));
    out.extend_from_slice(&[1, 0, 24, 0]);
    out.extend_from_slice(&le(0));
    out.extend_from_slice(&le(row * h));
    for v in [2835u32, 2835, 0, 0] {
        out.extend_from_slice(&le(v));
    }
    for y in (0..h as usize).rev() {
        let line = &rgba[y * w as usize * 4..][..w as usize * 4];
        for p in line.chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        out.resize(out.len() + (row - w * 3) as usize, 0);
    }
    std::fs::write(path, out)
}

/// Renders one frame of a scene offscreen and saves it (previews, visual tests).
fn shot(path: &str, (w, h): (u32, u32), warmup: f32, cfg: &Config, s: &config::Settings, bytes: &[u8]) -> Result<(), String> {
    let e = |x: windows::core::Error| x.message();
    let gpu = gfx::Gpu::new().map_err(e)?;
    let post = render::Post::new(&gpu).map_err(e)?;
    let scene = scene::parse(bytes)?;
    let mut renderer = render::Renderer::new(&gpu, &scene, scene.manifest.shadow.size).map_err(e)?;
    let mut world = world::World::new(scene.manifest);
    for _ in 0..((warmup * 30.0) as usize).max(1) {
        world.update(1.0 / 30.0, s);
    }
    renderer.prepare(&gpu, &mut world).map_err(e)?;
    let src = gfx::Target::new(&gpu, ((w as f32 * cfg.quality) as u32).max(16), ((h as f32 * cfg.quality) as u32).max(16)).map_err(e)?;
    let view = world.view(w as f32 / h as f32, false);
    renderer.render(&gpu, &world, &view, &src);
    let dst = gfx::Target::new(&gpu, w, h).map_err(e)?;
    post.draw(&gpu, &src.srv, (src.w, src.h), &dst.rtv, (w, h), [1.0, 1.0, 0.0, 0.0], cfg.fxaa, 0.28);
    let rgba = dst.read_rgba(&gpu).map_err(e)?;
    write_bmp(path, w, h, &rgba).map_err(|x| x.to_string())?;
    let (ws, private) = engine::memory();
    println!("saved {path} ({w}x{h}, hour {:.2}) | working set {ws:.0} MB | private {private:.0} MB", world.hour);
    Ok(())
}

static SINGLE_INSTANCE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Lets a freshly installed copy start while this process is still shutting down.
pub fn release_single_instance() {
    let h = SINGLE_INSTANCE.swap(0, Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = ReleaseMutex(HANDLE(h as *mut _));
            let _ = CloseHandle(HANDLE(h as *mut _));
        }
    }
}

/// The log of this process; it keeps the last run or two, so it cannot grow without limit.
fn open_log(name: &str) {
    let path = config::dir().join(name);
    let _ = std::fs::create_dir_all(config::dir());
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1 << 20) {
        let _ = std::fs::rename(&path, path.with_extension("old.log"));
    }
    *LOG.lock().unwrap() = std::fs::OpenOptions::new().create(true).append(true).open(path).ok();
}

fn main() {
    unsafe {
        // the setup runs from the Downloads folder: nothing may load from there
        let _ = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
    }
    std::panic::set_hook(Box::new(|p| log(&format!("kumo: crashed: {p}"))));
    let args = parse_args();
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    match args.mode.as_deref() {
        Some("--panel") => {
            open_log("panel.log");
            return panel::run_panel();
        }
        Some("--setup") => {
            open_log("setup.log");
            return setup::run_setup();
        }
        Some("--install") => {
            if !install::install_and_start() {
                unsafe { MessageBoxW(None, &HSTRING::from(tray::text_install_failed()), w!("Kumo"), MB_ICONWARNING) };
            }
            return;
        }
        Some("--uninstall") => return setup::run_uninstall(),
        _ => {}
    }
    if args.stats || args.shot.is_some() {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
    // the setup wizard greets people on GitHub, the panel does it in the Store package
    let first_run = install::packaged() && !config::config_path().exists();
    let mut cfg = Config::load();
    for (k, v) in &args.overrides {
        cfg.set(k, v);
    }

    if let Some(path) = &args.shot {
        let bytes = match &args.scene {
            Some(p) => std::fs::read(p).map(std::borrow::Cow::Owned).unwrap_or_else(|e| fail(&format!("{p}: {e}"))),
            None => std::borrow::Cow::Borrowed(library::PAGODA_SCENE),
        };
        let mut s = cfg.scene(config::BUILTIN_SCENE);
        for (k, v) in &args.overrides {
            s.set(k, v);
        }
        if let Err(e) = shot(path, args.size, args.warmup, &cfg, &s, &bytes) {
            eprintln!("kumo: {e}");
            std::process::exit(1);
        }
        return;
    }

    // any copy but the installed one is a download: show the setup
    if args.window.is_none() && !install::is_installed_copy() && std::env::var_os("KUMO_PORTABLE").is_none() {
        open_log("setup.log");
        return setup::run_setup();
    }

    unsafe {
        if args.window.is_none() {
            let mutex = CreateMutexW(None, true, w!("Local\\KumoWallpaperEngine"));
            if GetLastError() == ERROR_ALREADY_EXISTS {
                // started again (Start menu, desktop shortcut): open the control panel instead
                if let Some(h) = engine_window() {
                    let _ = PostMessageW(h, WM_KUMO_OPEN, WPARAM(0), LPARAM(0));
                }
                return;
            }
            if let Ok(m) = mutex {
                SINGLE_INSTANCE.store(m.0 as isize, Ordering::Relaxed);
            }
        }
        if LOG.lock().map(|l| l.is_none()).unwrap_or(false) {
            open_log("kumo.log");
        }
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let icon = tray::make_icon();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: desktop::CLASS,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
        let msg = CreateWindowExW(WS_EX_TOOLWINDOW, desktop::CLASS, w!("Kumo"), WS_POPUP, 0, 0, 0, 0, None, None, instance, None)
            .unwrap_or_else(|_| fail("could not create the message window"));
        let _ = WTSRegisterSessionNotification(msg, NOTIFY_FOR_THIS_SESSION);
        for guid in [GUID_CONSOLE_DISPLAY_STATE, GUID_POWER_SAVING_STATUS, GUID_ACDC_POWER_SOURCE] {
            let _ = RegisterPowerSettingNotification(HANDLE(msg.0), &guid, REGISTER_NOTIFICATION_FLAGS(0));
        }

        let gpu = gfx::Gpu::new().unwrap_or_else(|e| fail(&format!("Direct3D 11 is not available: {}", e.message())));
        let every = Duration::from_secs(if args.stats { 2 } else { 60 });
        let mut app = engine::App::new(gpu, cfg, instance, msg, args.window, every).unwrap_or_else(|e| fail(&e.message()));
        app.overrides = args.overrides.clone();
        log(&format!("Kumo {} - {} monitor(s)", env!("CARGO_PKG_VERSION"), desktop::monitors().len()));
        if args.window.is_none() {
            watch::hook();
        }
        app.build_outputs();
        // right after login Explorer may still be starting
        for _ in 0..20 {
            if !app.wants_outputs() || app.attach_denied {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
            app.build_outputs();
        }
        if app.wants_outputs() {
            // nothing could be shown: say why instead of running invisibly
            let recent = RECENT.lock().map(|r| r.join("\n")).unwrap_or_default();
            let first = if app.attach_denied { tray::text_attach_failed() } else { tray::text_no_output() };
            MessageBoxW(None, &HSTRING::from(format!("{first}\n\n{recent}")), w!("Kumo"), MB_ICONWARNING | MB_SETFOREGROUND);
        }
        app.tray = Some(tray::Tray::new(msg, icon, &tray::tooltip(app.cfg.paused)));
        if install::packaged() {
            // brings the wallpaper back after a Store update, and only then
            let _ = RegisterApplicationRestart(PCWSTR::null(), RESTART_NO_CRASH | RESTART_NO_HANG | RESTART_NO_REBOOT);
        }
        if first_run {
            app.cfg.save();
            start_panel(w!("--panel --welcome"));
        }
        engine::run(&mut app);
        if let Some(t) = &app.tray {
            t.remove();
        }
        app.destroy_outputs();
        if let Some(d) = &app.desktop {
            desktop::restore_wallpaper(d);
        }
        let _ = DestroyWindow(msg);
    }
}

slint::include_modules!();

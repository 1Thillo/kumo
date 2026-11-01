//! Control panel (`kumo.exe --panel`) and setup wizard. Both are separate, short-lived processes
//! drawn by Slint's software renderer, so they need no GPU context and give back everything on
//! close. The panel edits config.json and the library, then nudges the engine to reload.

use crate::config::{self, Config, Fit, Layout, PowerRule, TimeMode};
use crate::library::{self, Item, Kind};
use crate::{desktop, engine, install, IdentifyWindow, PanelWindow, Setup, SetupWindow, State};
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, Image, ModelRc, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

const LAPSE: [f32; 4] = [60.0, 240.0, 600.0, 2400.0];
const PETALS: [f32; 4] = [0.0, 0.5, 1.0, 1.6];
const FPS: [u32; 3] = [15, 30, 60];
const QUALITY: [f32; 3] = [1.0, 0.8, 0.62];
const SPEED: [f32; 4] = [0.5, 1.0, 1.5, 2.0];
const OTHER_APPS: [(&str, &str); 3] = [("lively.exe", "Lively Wallpaper"), ("wallpaper32.exe", "Wallpaper Engine"), ("wallpaper64.exe", "Wallpaper Engine")];

// ---------------------------------------------------------------- language

fn german(cfg: &Config) -> bool {
    match cfg.language.as_str() {
        "de" => true,
        "en" => false,
        _ => (unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff) == 0x07,
    }
}

fn apply_language(cfg: &Config) {
    let _ = slint::select_bundled_translation(if german(cfg) { "de" } else { "en" });
}

/// Small strings built on the Rust side (the .slint files use @tr and lang/de).
struct Tr(bool);

impl Tr {
    fn t(&self, de: &'static str, en: &'static str) -> &'static str {
        if self.0 { de } else { en }
    }
}

fn system_dark() -> bool {
    use windows::Win32::System::Registry::*;
    let mut v = 1u32;
    let mut len = 4u32;
    unsafe {
        let _ = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as *mut _),
            Some(&mut len),
        );
    }
    v == 0
}

fn dark(cfg: &Config) -> bool {
    match cfg.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => system_dark(),
    }
}

/// Dark or light system title bar to match the window. False while there is no native window yet.
fn title_bar(window: &slint::Window, dark: bool) -> bool {
    use winit::window::Theme;
    window.with_winit_window(|w| w.set_theme(Some(if dark { Theme::Dark } else { Theme::Light }))).is_some()
}

/// The native window only exists once the event loop runs.
fn title_bar_soon<T: ComponentHandle + 'static>(weak: slint::Weak<T>, dark: bool) {
    let timer = Rc::new(Timer::default());
    let t = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        if weak.upgrade().is_none_or(|app| title_bar(app.window(), dark)) {
            t.stop();
        }
    });
}

/// Windows 10 has neither of the two fonts the design uses.
fn apply_fonts(theme: crate::Theme) {
    use windows::Win32::UI::Shell::FOLDERID_Fonts;
    let fonts = install::known_folder(&FOLDERID_Fonts).unwrap_or_default();
    if !fonts.join("SegoeIcons.ttf").exists() {
        theme.set_icon_font("Segoe MDL2 Assets".into());
    }
    if !fonts.join("SegUIVar.ttf").exists() {
        theme.set_ui_font("Segoe UI".into());
    }
}

// ---------------------------------------------------------------- helpers

fn open(target: &str, args: Option<&str>) {
    let t = HSTRING::from(target);
    let a = args.map(HSTRING::from);
    unsafe {
        ShellExecuteW(None, w!("open"), &t, a.as_ref().map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr())), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

fn confirm(title: &str, text: &str) -> bool {
    unsafe { MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_YESNO | MB_ICONQUESTION | MB_SETFOREGROUND) == IDYES }
}

fn size_text(bytes: u64, de: bool) -> String {
    let text = if bytes >= 1 << 30 { format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64) } else { format!("{:.1} MB", bytes as f64 / (1u64 << 20) as f64) };
    if de { text.replace('.', ",") } else { text }
}

/// "1, 2 und 3"
fn list_text(items: &[String], tr: &Tr) -> String {
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} {} {last}", rest.join(", "), tr.t("und", "and")),
        _ => items.join(""),
    }
}

/// Newest last, so "0.2.0" compares with "0.10.0" as a person would expect.
fn version_key(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn running_processes() -> Vec<String> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    let mut names = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return names };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            names.push(String::from_utf16_lossy(&e.szExeFile[..n]).to_ascii_lowercase());
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    names
}

fn other_wallpaper_app() -> Option<&'static str> {
    let procs = running_processes();
    OTHER_APPS.iter().find(|(exe, _)| procs.iter().any(|p| p == exe)).map(|(_, name)| *name)
}

/// Wallpapers the engine could not show, with the reason.
fn failed_wallpapers() -> Vec<(String, String)> {
    std::fs::read_to_string(config::dir().join("status.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// A second panel start brings the open one to the front instead.
fn focus_existing(title: &str) -> bool {
    unsafe extern "system" fn cb(h: HWND, data: LPARAM) -> BOOL {
        let (title, pids, found) = &mut *(data.0 as *mut (Vec<u16>, Vec<u32>, HWND));
        let mut buf = [0u16; 64];
        let n = GetWindowTextW(h, &mut buf) as usize;
        let mut cls = [0u16; 32];
        let c = GetClassNameW(h, &mut cls) as usize;
        let mut pid = 0;
        GetWindowThreadProcessId(h, Some(&mut pid));
        if &buf[..n] == title.as_slice() && String::from_utf16_lossy(&cls[..c]) != "KumoWallpaper" && pids.contains(&pid) && IsWindowVisible(h).as_bool() {
            *found = h;
            return false.into();
        }
        true.into()
    }
    let mut data: (Vec<u16>, Vec<u32>, HWND) = (title.encode_utf16().collect(), install::running_copies(&[]), HWND::default());
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut data as *mut _ as isize));
        if !data.2.is_invalid() {
            crate::log(&format!("kumo: \"{title}\" is already open, showing it"));
            let _ = ShowWindow(data.2, SW_RESTORE);
            let _ = SetForegroundWindow(data.2);
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------- engine stats

struct EngineStats {
    pid: u32,
    last: Option<(Instant, f64)>,
}

impl EngineStats {
    /// (memory MB, CPU % of the PC) of the running engine.
    fn sample(&mut self) -> Option<(f64, f64)> {
        let h = crate::engine_window()?;
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        if pid != self.pid {
            self.pid = pid;
            self.last = None;
        }
        unsafe {
            let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mem = engine::process_memory(p).map(|m| m.1);
            let (mut c, mut e, mut k, mut u) = Default::default();
            let _ = GetProcessTimes(p, &mut c, &mut e, &mut k, &mut u);
            let _ = CloseHandle(p);
            let secs = |t: FILETIME| ((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) as f64 / 1e7;
            let now = (Instant::now(), secs(k) + secs(u));
            let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
            let cpu = self.last.map(|(t, s)| (now.1 - s) / now.0.duration_since(t).as_secs_f64().max(0.001) / cores * 100.0).unwrap_or(0.0);
            self.last = Some(now);
            Some((mem?, cpu))
        }
    }
}

// ---------------------------------------------------------------- panel

thread_local! {
    /// the panel's state, for callbacks that come back from worker threads
    static PANEL: RefCell<Option<Rc<RefCell<Ctx>>>> = const { RefCell::new(None) };
}

#[derive(PartialEq, Clone)]
enum Info {
    None,
    Welcome,
    EngineDown,
    OtherApp(&'static str),
    Failed(String, String),
    /// a file was imported: (id, title), with a button to use it
    Added(String, String),
    Message(i32, String),
}

struct Ctx {
    cfg: Config,
    thumbs: HashMap<PathBuf, (std::time::SystemTime, Image)>,
    dropped: Vec<PathBuf>,
    info: Info,
    dismissed: Vec<Info>,
    detail_id: String,
    stats: EngineStats,
    tr: Tr,
    /// looked up every 30 s, a process list is too much for the 2 second tick
    other_app: Option<&'static str>,
    other_app_at: Option<Instant>,
    items: usize,
    config_time: Option<std::time::SystemTime>,
    identify: Vec<IdentifyWindow>,
}

impl Ctx {
    fn thumb(&mut self, item: &Item) -> Option<Image> {
        let p = item.thumbnail();
        let modified = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
        if let Some((t, img)) = self.thumbs.get(&p) {
            if *t == modified {
                return Some(img.clone());
            }
        }
        let img = Image::load_from_path(&p).ok()?;
        self.thumbs.insert(p, (modified, img.clone()));
        Some(img)
    }

    fn save(&mut self) {
        self.cfg.save();
        self.config_time = config_time();
        crate::notify_engine();
    }

    fn other_app(&mut self, fresh: bool) -> Option<&'static str> {
        if fresh || self.other_app_at.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            self.other_app = other_wallpaper_app();
            self.other_app_at = Some(Instant::now());
        }
        self.other_app
    }
}

fn config_time() -> Option<std::time::SystemTime> {
    std::fs::metadata(config::config_path()).and_then(|m| m.modified()).ok()
}

fn kind_index(k: Kind) -> i32 {
    match k {
        Kind::Scene => 0,
        Kind::Video => 1,
        Kind::Image => 2,
    }
}

fn meta(tr: &Tr, item: &Item) -> String {
    let size = || std::fs::metadata(item.path()).map(|m| size_text(m.len(), tr.0)).unwrap_or_default();
    match item.kind {
        Kind::Scene if item.builtin => format!("{} · {}", tr.t("3D-Szene", "3D scene"), tr.t("Eingebaut", "Built-in")),
        Kind::Scene => format!("{} · {}", tr.t("3D-Szene", "3D scene"), size()),
        Kind::Video => format!("Video · {}", size()),
        Kind::Image => match crate::media::image_size(&item.path()) {
            Ok((w, h)) => format!("{} · {w} × {h}", tr.t("Bild", "Picture")),
            Err(_) => tr.t("Bild", "Picture").to_string(),
        },
    }
}

fn refresh(app: &PanelWindow, ctx: &mut Ctx) {
    let st = app.global::<State>();
    let items = library::list();
    let mons = desktop::monitors();
    let cfg = ctx.cfg.clone();
    let tr = Tr(ctx.tr.0);

    // which screens show what
    let failed = failed_wallpapers();
    let shown: Vec<String> = mons
        .iter()
        .map(|m| {
            let w = cfg.wallpaper_for(&m.id);
            if failed.iter().any(|(id, _)| id == w) { config::BUILTIN_SCENE.to_string() } else { w.to_string() }
        })
        .collect();
    let tiles: Vec<crate::TileData> = items
        .iter()
        .map(|it| {
            let on: Vec<String> = shown.iter().enumerate().filter(|(_, w)| **w == it.id).map(|(i, _)| (i + 1).to_string()).collect();
            let thumb = ctx.thumb(it);
            crate::TileData {
                id: it.id.clone().into(),
                title: it.title.clone().into(),
                meta: meta(&tr, it).into(),
                kind: kind_index(it.kind),
                has_thumb: thumb.is_some(),
                thumb: thumb.unwrap_or_default(),
                active: !on.is_empty(),
                active_label: if on.is_empty() || (mons.len() > 1 && on.len() == mons.len()) || mons.len() <= 1 {
                    tr.t("Aktiv", "Active").into()
                } else {
                    format!("{} · {}", tr.t("Aktiv", "Active"), on.join(", ")).into()
                },
                builtin: it.builtin,
            }
        })
        .collect();
    st.set_tiles(ModelRc::new(VecModel::from(tiles)));
    ctx.items = items.len();

    // monitor canvas: rectangles as fractions of the bounding box
    let (x0, y0) = (mons.iter().map(|m| m.rect.left).min().unwrap_or(0), mons.iter().map(|m| m.rect.top).min().unwrap_or(0));
    let (x1, y1) = (mons.iter().map(|m| m.rect.right).max().unwrap_or(1), mons.iter().map(|m| m.rect.bottom).max().unwrap_or(1));
    let (bw, bh) = (((x1 - x0).max(1)) as f32, ((y1 - y0).max(1)) as f32);
    let screens: Vec<crate::ScreenData> = mons
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let item = items.iter().find(|it| it.id == shown[i]).cloned().unwrap_or_else(|| items[0].clone());
            let thumb = ctx.thumb(&item);
            crate::ScreenData {
                label: (i + 1).to_string().into(),
                name: if tr.0 { m.name.replace("Display ", "Bildschirm ") } else { m.name.clone() }.into(),
                detail: format!("{} × {}", m.w(), m.h()).into(),
                fx: (m.rect.left - x0) as f32 / bw,
                fy: (m.rect.top - y0) as f32 / bh,
                fw: m.w() as f32 / bw,
                fh: m.h() as f32 / bh,
                has_thumb: thumb.is_some(),
                thumb: thumb.unwrap_or_default(),
                wallpaper: item.title.clone().into(),
                kind: kind_index(item.kind),
                primary: m.primary,
                off: cfg.off.contains(&m.id),
            }
        })
        .collect();
    let n = screens.len().max(1) as i32;
    st.set_screens(ModelRc::new(VecModel::from(screens)));
    st.set_layout_aspect(bw / bh);
    if st.get_selected_screen() >= n {
        st.set_selected_screen(0);
    }
    st.set_screen_on(mons.get(st.get_selected_screen() as usize).is_none_or(|m| !cfg.off.contains(&m.id)));
    st.set_per_monitor(cfg.layout == Layout::PerMonitor);

    // now playing
    let distinct: Vec<&String> = shown.iter().fold(Vec::new(), |mut v, w| {
        if !v.contains(&w) {
            v.push(w);
        }
        v
    });
    let first = items.iter().find(|it| Some(&it.id) == distinct.first().copied()).cloned().unwrap_or_else(|| items[0].clone());
    if distinct.len() > 1 {
        st.set_now_title(format!("{} {}", distinct.len(), tr.t("Hintergründe aktiv", "wallpapers active")).into());
    } else {
        st.set_now_title(first.title.clone().into());
    }
    let now_thumb = ctx.thumb(&first);
    st.set_now_has_thumb(now_thumb.is_some());
    st.set_now_kind(kind_index(first.kind));
    st.set_now_thumb(now_thumb.unwrap_or_default());
    st.set_now_sub(
        if mons.len() > 1 { format!("{} · {} fps", tr.t("Läuft auf allen Bildschirmen", "Running on all screens"), cfg.fps) } else { format!("{} · {} fps", tr.t("Läuft", "Running"), cfg.fps) }.into(),
    );
    st.set_paused(cfg.paused);

    // playback & performance
    st.set_fps_index(FPS.iter().position(|&f| f == cfg.fps).unwrap_or(1) as i32);
    st.set_quality_index(QUALITY.iter().position(|&q| (q - cfg.quality).abs() < 0.01).unwrap_or(0) as i32);
    st.set_fxaa(cfg.fxaa);
    st.set_pause_covered(cfg.pause_when_covered);
    let rule = |r: PowerRule| match r {
        PowerRule::Keep => 0,
        PowerRule::Half => 1,
        PowerRule::Pause => 2,
    };
    st.set_battery_index(rule(cfg.on_battery));
    st.set_saver_index(rule(cfg.energy_saver));
    st.set_has_battery(has_battery());

    // settings
    st.set_autostart(config::autostart());
    st.set_language_index(match cfg.language.as_str() {
        "de" => 1,
        "en" => 2,
        _ => 0,
    });
    st.set_theme_index(match cfg.theme.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    });
    st.set_library_path(library::dir().display().to_string().into());
    st.set_library_size(size_text(install::data_size(), tr.0).into());
    st.set_version(env!("CARGO_PKG_VERSION").into());

    if !ctx.detail_id.is_empty() {
        fill_detail(app, ctx);
    }
    show_info(app, ctx);
}

fn has_battery() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.BatteryFlag != 128 }
}

fn fill_detail(app: &PanelWindow, ctx: &mut Ctx) {
    let st = app.global::<State>();
    let Some(item) = library::get(&ctx.detail_id) else {
        ctx.detail_id.clear();
        st.set_page(0);
        return;
    };
    let tr = Tr(ctx.tr.0);
    let thumb = ctx.thumb(&item);
    let mons = desktop::monitors();
    let on: Vec<String> = mons.iter().enumerate().filter(|(_, m)| ctx.cfg.wallpaper_for(&m.id) == item.id).map(|(i, _)| (i + 1).to_string()).collect();
    st.set_detail(crate::TileData {
        id: item.id.clone().into(),
        title: item.title.clone().into(),
        meta: meta(&tr, &item).into(),
        kind: kind_index(item.kind),
        has_thumb: thumb.is_some(),
        thumb: thumb.unwrap_or_default(),
        active: !on.is_empty(),
        active_label: tr.t("Aktiv", "Active").into(),
        builtin: item.builtin,
    });
    st.set_detail_type(
        match item.kind {
            Kind::Scene => tr.t("3D-Szene (.kumo)", "3D scene (.kumo)"),
            Kind::Video => "Video",
            Kind::Image => tr.t("Bild", "Picture"),
        }
        .into(),
    );
    st.set_detail_source(if item.builtin { tr.t("Eingebaut", "Built-in").to_string() } else { meta(&tr, &item) }.into());
    st.set_detail_memory(
        match item.kind {
            Kind::Scene => tr.t("ca. 55–75 MB gesamt", "about 55–75 MB in total"),
            Kind::Video => tr.t("ca. +30–60 MB", "about +30–60 MB"),
            Kind::Image => tr.t("ca. 25 MB gesamt, 0 % CPU", "about 25 MB in total, 0 % CPU"),
        }
        .into(),
    );
    st.set_detail_status(
        if on.is_empty() {
            tr.t("Nicht aktiv – Einstellungen gelten beim Anwenden", "Not active – settings apply once you use it").to_string()
        } else if mons.len() > 1 {
            format!("{} {}", tr.t("Läuft auf Bildschirm", "Running on screen"), list_text(&on, &tr))
        } else {
            tr.t("Läuft", "Running").to_string()
        }
        .into(),
    );
    let s = ctx.cfg.scene(&item.id);
    st.set_time_mode(match s.time {
        TimeMode::Real => 0,
        TimeMode::Fixed => 1,
        TimeMode::Lapse => 2,
    });
    st.set_hour(s.hour);
    st.set_lapse_index(LAPSE.iter().position(|&l| (l - s.lapse).abs() < 1.0).unwrap_or(2) as i32);
    st.set_rotate(s.rotate);
    st.set_petals_index(PETALS.iter().position(|&p| (p - s.petals).abs() < 0.05).unwrap_or(2) as i32);
    st.set_parallax(s.parallax);
    let media = ctx.cfg.media(&item.id);
    st.set_fit_index(match media.fit {
        Fit::Cover => 0,
        Fit::Contain => 1,
        Fit::Stretch => 2,
    });
    st.set_speed_index(SPEED.iter().position(|&s| (s - media.speed).abs() < 0.01).unwrap_or(1) as i32);
}

fn show_info(app: &PanelWindow, ctx: &mut Ctx) {
    let st = app.global::<State>();
    let tr = Tr(ctx.tr.0);
    let running = crate::engine_window().is_some();
    st.set_running(running);
    let failed = failed_wallpapers();
    let next = if matches!(ctx.info, Info::Message(..) | Info::Added(..)) {
        ctx.info.clone()
    } else if !running {
        Info::EngineDown
    } else if let Some((id, why)) = failed.first() {
        Info::Failed(library::get(id).map_or_else(|| id.clone(), |i| i.title), why.clone())
    } else if let Some(app_name) = ctx.other_app(false) {
        Info::OtherApp(app_name)
    } else if ctx.items <= 1 {
        Info::Welcome
    } else {
        Info::None
    };
    ctx.info = if ctx.dismissed.contains(&next) { Info::None } else { next };
    let (kind, text, action): (i32, String, &str) = match &ctx.info {
        Info::None => (0, String::new(), ""),
        Info::Welcome => (1, tr.t("Willkommen bei Kumo – ziehe ein Video oder Bild in dieses Fenster, um es als Hintergrund zu verwenden.", "Welcome to Kumo – drag a video or picture into this window to use it as your wallpaper.").into(), ""),
        Info::EngineDown => (3, tr.t("Kumo läuft gerade nicht – dein Live-Hintergrund ist aus.", "Kumo is not running – your live wallpaper is off.").into(), tr.t("Kumo starten", "Start Kumo")),
        Info::OtherApp(name) => (
            2,
            if tr.0 {
                format!("{name} läuft ebenfalls. Zwei Hintergrund-Programme stören sich gegenseitig – beende {name} über sein Symbol im Infobereich.")
            } else {
                format!("{name} is running too. Two wallpaper programs get in each other's way – quit {name} from its icon in the notification area.")
            },
            tr.t("Erneut prüfen", "Check again"),
        ),
        Info::Failed(title, why) => (
            3,
            if tr.0 {
                format!("„{title}“ konnte nicht angezeigt werden, darum läuft die eingebaute Szene. Grund: {why}")
            } else {
                format!("“{title}” could not be shown, so the built-in scene is running. Reason: {why}")
            },
            if why.contains("decod") { tr.t("Im Store suchen", "Search the Store") } else { "" },
        ),
        Info::Added(_, title) => (
            4,
            if tr.0 { format!("„{title}“ wurde hinzugefügt.") } else { format!("“{title}” was added.") },
            tr.t("Jetzt anwenden", "Apply now"),
        ),
        Info::Message(k, m) => (*k, m.clone(), ""),
    };
    st.set_info_kind(kind);
    st.set_info_text(text.into());
    st.set_info_action(action.into());
}

fn start_engine() {
    let exe = install::installed_exe();
    let exe = if exe.exists() { exe } else { std::env::current_exe().unwrap_or_default() };
    open(&exe.display().to_string(), None);
}

/// Asks the engine to quit and waits for it.
fn stop_engine() {
    if let Some(h) = crate::engine_window() {
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(h, Some(&mut pid));
            let _ = PostMessageW(h, WM_CLOSE, WPARAM(0), LPARAM(0));
            if let Ok(p) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
                WaitForSingleObject(p, 5000);
                let _ = CloseHandle(p);
            }
        }
    }
}

fn apply_everywhere(cfg: &mut Config, id: &str) {
    cfg.wallpaper = id.to_string();
    for m in desktop::monitors() {
        cfg.monitors.insert(m.id, id.to_string());
    }
}

/// Shows each screen's number on that screen for a moment.
fn identify(ctx: &Rc<RefCell<Ctx>>) {
    let mut windows = Vec::new();
    for (i, m) in desktop::monitors().iter().enumerate() {
        let Ok(w) = IdentifyWindow::new() else { continue };
        w.set_label((i + 1).to_string().into());
        w.window().set_position(slint::PhysicalPosition::new(m.rect.left + 48, m.rect.top + 48));
        let _ = w.show();
        windows.push(w);
    }
    ctx.borrow_mut().identify = windows;
    let ctx = ctx.clone();
    Timer::single_shot(Duration::from_secs(2), move || {
        for w in std::mem::take(&mut ctx.borrow_mut().identify) {
            let _ = w.hide();
        }
    });
}

/// Copies files into the library (on a worker thread) and makes thumbnails.
fn import(weak: slint::Weak<PanelWindow>, ctx: Rc<RefCell<Ctx>>, files: Vec<PathBuf>) {
    let de = ctx.borrow().tr.0;
    let (files, skipped): (Vec<PathBuf>, Vec<PathBuf>) = files.into_iter().partition(|p| library::kind_of(p).is_some());
    let name = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut errors: Vec<String> = skipped.iter().map(|p| format!("{}: {}", name(p), Tr(de).t("Dieses Format wird nicht unterstützt.", "This format is not supported."))).collect();
    if files.is_empty() {
        if let (false, Some(app)) = (errors.is_empty(), weak.upgrade()) {
            let mut c = ctx.borrow_mut();
            c.info = Info::Message(2, errors.join("\n"));
            show_info(&app, &mut c);
        }
        return;
    }
    let label = Tr(de).t("Wird hinzugefügt …", "Adding…");
    if let Some(app) = weak.upgrade() {
        app.global::<State>().set_busy_text(label.into());
    }
    std::thread::spawn(move || {
        let mut added = None;
        for f in &files {
            let (w, mut shown) = (weak.clone(), 0);
            let progress = move |part: f32| {
                let percent = (part * 100.0) as u32;
                if percent != shown {
                    shown = percent;
                    let w = w.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = w.upgrade() {
                            app.global::<State>().set_busy_text(format!("{label} {percent} %").into());
                        }
                    });
                }
            };
            match library::import(f, progress) {
                Ok(item) => {
                    let r = match item.kind {
                        Kind::Image => crate::media::thumbnail_from_file(&item.path(), &item.thumbnail()),
                        _ => crate::media::thumbnail_via_shell(&item.path(), &item.thumbnail()),
                    };
                    if let Err(e) = r {
                        crate::log(&format!("kumo: no thumbnail for {}: {}", item.title, e.message()));
                    }
                    added = Some(item);
                }
                Err(e) => errors.push(format!("{}: {e}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
            }
        }
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(ctx) = PANEL.with(|p| p.borrow().clone()) else { return };
            let mut c = ctx.borrow_mut();
            app.global::<State>().set_busy_text("".into());
            if !errors.is_empty() {
                c.info = Info::Message(3, errors.join("\n"));
            } else if let Some(item) = added {
                if c.items <= 1 {
                    // the very first own wallpaper is what the user came for
                    c.cfg = Config::load();
                    apply_everywhere(&mut c.cfg, &item.id);
                    c.save();
                    let text = if c.tr.0 { format!("„{}“ ist jetzt dein Hintergrund.", item.title) } else { format!("“{}” is now your wallpaper.", item.title) };
                    c.info = Info::Message(4, text);
                } else {
                    c.info = Info::Added(item.id, item.title);
                }
            }
            refresh(&app, &mut c);
        });
    });
}
/// Native multi-select file picker.
fn pick_files(de: bool) -> Vec<PathBuf> {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::*;
    let mut out = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(dlg) = CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) else { return out };
        let name = HSTRING::from(if de { "Videos, Bilder und Kumo-Szenen" } else { "Videos, pictures and Kumo scenes" });
        let spec = HSTRING::from("*.mp4;*.m4v;*.mov;*.webm;*.mkv;*.wmv;*.avi;*.jpg;*.jpeg;*.png;*.bmp;*.webp;*.tif;*.tiff;*.heic;*.avif;*.kumo");
        let filters = [COMDLG_FILTERSPEC { pszName: PCWSTR(name.as_ptr()), pszSpec: PCWSTR(spec.as_ptr()) }];
        let _ = dlg.SetFileTypes(&filters);
        let _ = dlg.SetOptions(dlg.GetOptions().unwrap_or_default() | FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST);
        if dlg.Show(None).is_err() {
            return out;
        }
        let Ok(items) = dlg.GetResults() else { return out };
        for i in 0..items.GetCount().unwrap_or(0) {
            if let Ok(item) = items.GetItemAt(i) {
                if let Ok(p) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                    out.push(PathBuf::from(p.to_string().unwrap_or_default()));
                    windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
                }
            }
        }
    }
    out
}

pub fn run_panel() {
    if focus_existing("Kumo") {
        return;
    }
    library::sweep();
    let cfg = Config::load();
    let app = match PanelWindow::new() {
        Ok(a) => a,
        Err(e) => crate::fail(&format!("The control panel could not open: {e}")),
    };
    apply_language(&cfg);
    apply_fonts(app.global::<crate::Theme>());
    app.invoke_set_dark(dark(&cfg));
    let ctx = Rc::new(RefCell::new(Ctx {
        tr: Tr(german(&cfg)),
        cfg,
        thumbs: HashMap::new(),
        dropped: Vec::new(),
        info: Info::None,
        dismissed: Vec::new(),
        detail_id: String::new(),
        stats: EngineStats { pid: 0, last: None },
        other_app: None,
        other_app_at: None,
        items: 0,
        config_time: config_time(),
        identify: Vec::new(),
    }));
    refresh(&app, &mut ctx.borrow_mut());
    PANEL.with(|p| *p.borrow_mut() = Some(ctx.clone()));
    let st = app.global::<State>();
    let weak = app.as_weak();

    // a change made in the panel: update config, tell the engine, redraw the panel
    let change = {
        let (weak, ctx) = (weak.clone(), ctx.clone());
        move |f: &dyn Fn(&PanelWindow, &mut Ctx)| {
            let Some(app) = weak.upgrade() else { return };
            let mut c = ctx.borrow_mut();
            c.cfg = Config::load(); // the tray menu may have changed something since
            f(&app, &mut c);
            c.save();
            refresh(&app, &mut c);
        }
    };
    let change = Rc::new(change);

    let ch = change.clone();
    st.on_apply(move |id, screen| {
        ch(&|app, c| {
            let st = app.global::<State>();
            let mons = desktop::monitors();
            if screen < 0 {
                apply_everywhere(&mut c.cfg, &id);
            } else if let Some(m) = mons.get(screen as usize) {
                if c.cfg.layout == Layout::Duplicate {
                    // one screen was picked: the others keep what they show
                    let wp = c.cfg.wallpaper.clone();
                    mons.iter().for_each(|m| drop(c.cfg.monitors.insert(m.id.clone(), wp.clone())));
                    c.cfg.layout = Layout::PerMonitor;
                    c.info = Info::Message(1, c.tr.t("Jeder Bildschirm hat jetzt seinen eigenen Hintergrund.", "Each screen now has its own wallpaper.").into());
                }
                c.cfg.monitors.insert(m.id.clone(), id.to_string());
            }
            if st.get_pick_mode() {
                st.set_pick_mode(false);
                st.set_page(2);
            }
        });
    });

    let (w2, c2) = (weak.clone(), ctx.clone());
    st.on_customize(move |id| {
        let Some(app) = w2.upgrade() else { return };
        let mut c = c2.borrow_mut();
        let id = if id.is_empty() { c.cfg.wallpaper.clone() } else { id.to_string() };
        c.detail_id = id;
        fill_detail(&app, &mut c);
        app.global::<State>().set_page(1);
    });

    let (ch, w3) = (change.clone(), weak.clone());
    st.on_remove(move |id| {
        let id = id.to_string();
        let Some(item) = library::get(&id) else { return };
        let de = german(&Config::load());
        let title = if de { format!("„{}“ entfernen?", item.title) } else { format!("Remove “{}”?", item.title) };
        let body = if de { "Die Kopie in der Kumo-Bibliothek wird gelöscht. Deine Originaldatei bleibt erhalten." } else { "The copy in the Kumo library is deleted. Your original file stays where it is." };
        if !confirm(&title, body) {
            return;
        }
        ch(&|app, c| {
            if c.cfg.wallpaper == id {
                c.cfg.wallpaper = config::BUILTIN_SCENE.into();
            }
            for v in c.cfg.monitors.values_mut() {
                if *v == id {
                    *v = config::BUILTIN_SCENE.into();
                }
            }
            c.cfg.media.remove(&id);
            c.cfg.scenes.remove(&id);
            c.items = c.items.saturating_sub(1);
            if c.detail_id == id {
                c.detail_id.clear();
                app.global::<State>().set_page(0);
            }
        });
        // after the save: the engine has to let go of the file first
        let weak = w3.clone();
        std::thread::spawn(move || {
            let result = library::remove(&id);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = weak.upgrade() else { return };
                let Some(ctx) = PANEL.with(|p| p.borrow().clone()) else { return };
                let mut c = ctx.borrow_mut();
                if let Err(e) = result {
                    c.info = Info::Message(3, e);
                }
                refresh(&app, &mut c);
            });
        });
    });

    st.on_explorer(move |id| {
        if let Some(item) = library::get(&id) {
            let target = if item.builtin { item.folder() } else { item.path() };
            open("explorer.exe", Some(&format!("/select,\"{}\"", target.display())));
        }
    });

    let (w2, c2) = (weak.clone(), ctx.clone());
    st.on_add_files(move || {
        let de = c2.borrow().tr.0;
        let files = pick_files(de);
        import(w2.clone(), c2.clone(), files);
    });

    let ch = change.clone();
    st.on_toggle_pause(move || ch(&|_, c| c.cfg.paused = !c.cfg.paused));

    let ch = change.clone();
    st.on_scene_changed(move || {
        ch(&|app, c| {
            let st = app.global::<State>();
            let id = c.detail_id.clone();
            let s = c.cfg.scene_mut(&id);
            s.time = match st.get_time_mode() {
                1 => TimeMode::Fixed,
                2 => TimeMode::Lapse,
                _ => TimeMode::Real,
            };
            s.hour = st.get_hour();
            s.lapse = LAPSE[st.get_lapse_index().clamp(0, 3) as usize];
            s.rotate = st.get_rotate();
            s.petals = PETALS[st.get_petals_index().clamp(0, 3) as usize];
            s.parallax = st.get_parallax();
        })
    });

    let ch = change.clone();
    st.on_reset_scene(move || {
        ch(&|_, c| {
            let id = c.detail_id.clone();
            c.cfg.scenes.remove(&id);
        })
    });

    let ch = change.clone();
    st.on_media_changed(move || {
        ch(&|app, c| {
            let id = c.detail_id.clone();
            let st = app.global::<State>();
            let m = c.cfg.media.entry(id).or_default();
            m.fit = match st.get_fit_index() {
                1 => Fit::Contain,
                2 => Fit::Stretch,
                _ => Fit::Cover,
            };
            m.speed = SPEED[st.get_speed_index().clamp(0, 3) as usize];
        })
    });

    let ch = change.clone();
    st.on_perf_changed(move || {
        ch(&|app, c| {
            let st = app.global::<State>();
            c.cfg.fps = FPS[st.get_fps_index().clamp(0, 2) as usize];
            c.cfg.quality = QUALITY[st.get_quality_index().clamp(0, 2) as usize];
            c.cfg.fxaa = st.get_fxaa();
            c.cfg.pause_when_covered = st.get_pause_covered();
            let rule = |i: i32| match i {
                0 => PowerRule::Keep,
                2 => PowerRule::Pause,
                _ => PowerRule::Half,
            };
            c.cfg.on_battery = rule(st.get_battery_index());
            c.cfg.energy_saver = rule(st.get_saver_index());
        })
    });

    let ch = change.clone();
    st.on_settings_changed(move || {
        ch(&|app, c| {
            let st = app.global::<State>();
            if st.get_autostart() != config::autostart() {
                config::set_autostart(st.get_autostart());
            }
            c.cfg.language = ["system", "de", "en"][st.get_language_index().clamp(0, 2) as usize].into();
            c.cfg.theme = ["system", "light", "dark"][st.get_theme_index().clamp(0, 2) as usize].into();
            c.tr = Tr(german(&c.cfg));
            apply_language(&c.cfg);
            let d = dark(&c.cfg);
            app.invoke_set_dark(d);
            title_bar(app.window(), d);
        })
    });

    let ch = change.clone();
    st.on_layout_changed(move || {
        ch(&|app, c| {
            let per = app.global::<State>().get_per_monitor();
            c.cfg.layout = if per { Layout::PerMonitor } else { Layout::Duplicate };
            if per {
                let wp = c.cfg.wallpaper.clone();
                for m in desktop::monitors() {
                    c.cfg.monitors.entry(m.id).or_insert_with(|| wp.clone());
                }
            }
        })
    });

    let c2 = ctx.clone();
    st.on_identify(move || identify(&c2));

    let ch = change.clone();
    st.on_screen_toggled(move |screen, on| {
        ch(&|_, c| {
            let Some(m) = desktop::monitors().into_iter().nth(screen as usize) else { return };
            c.cfg.off.retain(|id| *id != m.id);
            if !on {
                c.cfg.off.push(m.id);
            }
        })
    });

    let w2 = weak.clone();
    st.on_pick_for_screen(move || {
        if let Some(app) = w2.upgrade() {
            let st = app.global::<State>();
            st.set_pick_mode(true);
            st.set_page(0);
        }
    });
    let w2 = weak.clone();
    st.on_cancel_pick(move || {
        if let Some(app) = w2.upgrade() {
            let st = app.global::<State>();
            st.set_pick_mode(false);
            st.set_page(2);
        }
    });

    let (w2, c2) = (weak.clone(), ctx.clone());
    st.on_info_action_clicked(move || {
        let Some(app) = w2.upgrade() else { return };
        let mut c = c2.borrow_mut();
        match c.info.clone() {
            Info::EngineDown => start_engine(),
            Info::Failed(..) => open("ms-windows-store://search/?query=video%20extensions", None),
            Info::Added(id, _) => {
                c.cfg = Config::load();
                apply_everywhere(&mut c.cfg, &id);
                c.save();
                refresh(&app, &mut c);
            }
            _ => {}
        }
        c.info = Info::None;
        c.other_app(true);
        show_info(&app, &mut c);
    });
    let (w2, c2) = (weak.clone(), ctx.clone());
    st.on_info_dismissed(move || {
        let Some(app) = w2.upgrade() else { return };
        let mut c = c2.borrow_mut();
        let i = c.info.clone();
        if !matches!(i, Info::Message(..)) {
            c.dismissed.push(i);
        }
        c.info = Info::None;
        show_info(&app, &mut c);
    });

    st.on_open_log(|| open(&config::dir().join("kumo.log").display().to_string(), None));
    st.on_open_library(|| {
        let _ = std::fs::create_dir_all(library::dir());
        open(&library::dir().display().to_string(), None)
    });
    st.on_restart_engine(|| {
        stop_engine();
        start_engine();
    });
    st.on_quit_engine(stop_engine);
    let ch = change.clone();
    st.on_reset_all(move || {
        let de = german(&Config::load());
        let (t, b) = if de {
            ("Alle Einstellungen zurücksetzen?", "Bildrate, Qualität, Pausenregeln, Bildschirme und alle Anpassungen werden auf den Standard gesetzt, und die Pagode wird wieder dein Hintergrund. Deine Bibliothek bleibt erhalten.")
        } else {
            ("Reset all settings?", "Frame rate, quality, pause rules, screens and everything you customized go back to the defaults, and the pagoda becomes your wallpaper again. Your library stays.")
        };
        if confirm(t, b) {
            ch(&|_, c| {
                let (lang, theme) = (c.cfg.language.clone(), c.cfg.theme.clone());
                c.cfg = Config { language: lang, theme, ..Config::default() };
            });
        }
    });
    st.on_uninstall(|| {
        let exe = install::installed_exe();
        if exe.exists() {
            open(&exe.display().to_string(), Some("--uninstall"));
        } else {
            open("ms-settings:appsfeatures", None);
        }
    });
    st.on_open_license(|| {
        let local = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("LICENSE.txt"))).filter(|p| p.exists());
        match local {
            Some(p) => open(&p.display().to_string(), None),
            None => open("https://www.gnu.org/licenses/gpl-3.0.html", None),
        }
    });

    // drag & drop from Explorer
    let (w2, c2) = (weak.clone(), ctx.clone());
    app.window().on_winit_window_event(move |_, ev| {
        let Some(app) = w2.upgrade() else { return EventResult::Propagate };
        let st = app.global::<State>();
        match ev {
            winit::event::WindowEvent::HoveredFile(p) => {
                st.set_dragging(true);
                st.set_drag_ok(st.get_drag_ok() && library::kind_of(p).is_some() || !st.get_dragging());
                if library::kind_of(p).is_none() {
                    st.set_drag_ok(false);
                }
            }
            winit::event::WindowEvent::HoveredFileCancelled => {
                st.set_dragging(false);
                st.set_drag_ok(true);
            }
            winit::event::WindowEvent::ThemeChanged(_) if c2.borrow().cfg.theme == "system" => {
                app.invoke_set_dark(system_dark());
                title_bar(app.window(), system_dark());
            }
            winit::event::WindowEvent::DroppedFile(p) => {
                st.set_dragging(false);
                st.set_drag_ok(true);
                let first = c2.borrow().dropped.is_empty();
                c2.borrow_mut().dropped.push(p.clone());
                if first {
                    // winit reports one event per file: collect them, then import once
                    let (w3, c3) = (w2.clone(), c2.clone());
                    Timer::single_shot(Duration::from_millis(80), move || {
                        let files = std::mem::take(&mut c3.borrow_mut().dropped);
                        import(w3.clone(), c3.clone(), files);
                    });
                }
            }
            _ => {}
        }
        EventResult::Propagate
    });

    // live numbers every 2 s (the only periodic redraw)
    let (w2, c2) = (weak.clone(), ctx.clone());
    let timer = Timer::default();
    let tick = move || {
        let Some(app) = w2.upgrade() else { return };
        if app.window().is_minimized() {
            return;
        }
        let st = app.global::<State>();
        let mut c = c2.borrow_mut();
        // the tray menu writes config.json too
        if config_time() != c.config_time {
            c.config_time = config_time();
            c.cfg = Config::load();
            refresh(&app, &mut c);
        }
        match c.stats.sample() {
            Some((mem, cpu)) => {
                st.set_ram(format!("{mem:.0} MB").into());
                let text = format!("{cpu:.1} %");
                st.set_cpu(if c.tr.0 { text.replace('.', ",") } else { text }.into());
            }
            None => {
                st.set_ram("–".into());
                st.set_cpu("–".into());
            }
        }
        let own = engine::memory().1;
        st.set_panel_ram(format!("{own:.0} MB").into());
        let was = st.get_running();
        show_info(&app, &mut c);
        if was != st.get_running() {
            refresh(&app, &mut c);
        }
    };
    tick();
    timer.start(TimerMode::Repeated, Duration::from_secs(2), tick);

    title_bar_soon(weak.clone(), dark(&ctx.borrow().cfg));

    // jump straight to a page (visual tests)
    if let Some(p) = std::env::var("KUMO_UI_PAGE").ok().and_then(|v| v.parse::<i32>().ok()) {
        if p == 1 {
            st.invoke_customize("".into());
        } else {
            st.set_page(p);
        }
    }

    let _ = app.run();
    drop(timer);
}

// ---------------------------------------------------------------- setup wizard

/// Waits for the installed engine, then shows "Kumo is running".
fn wait_for_engine(weak: slint::Weak<SetupWindow>) {
    let started = Instant::now();
    let timer = Rc::new(Timer::default());
    let t = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        let Some(app) = weak.upgrade() else { return };
        let s = app.global::<Setup>();
        if crate::engine_window().is_some() {
            s.set_page(6);
            t.stop();
        } else if started.elapsed() > Duration::from_secs(10) {
            let de = german(&Config::load());
            s.set_error_text(Tr(de).t("Kumo ist installiert, aber nicht gestartet. Öffne es im Startmenü.", "Kumo is installed but did not start. Open it from the Start menu.").into());
            s.set_page(3);
            t.stop();
        }
    });
}

pub fn run_setup() {
    if ["Kumo installieren", "Install Kumo", "Kumo aktualisieren", "Update Kumo", "Kumo-Einrichtung", "Kumo setup", "Kumo deinstallieren", "Uninstall Kumo"].into_iter().any(focus_existing) {
        return;
    }
    let (app, cfg) = setup_window();
    let tr = Tr(german(&cfg));
    let s = app.global::<Setup>();
    s.set_version(env!("CARGO_PKG_VERSION").into());
    let installed = install::installed_version();
    if let Some(v) = &installed {
        // nothing to update: the page offers to open, repair or remove Kumo instead
        let own = version_key(env!("CARGO_PKG_VERSION"));
        if version_key(v) >= own {
            s.set_newer_installed(version_key(v) > own);
            s.set_page(7);
        }
    }
    s.set_installed_version(installed.unwrap_or_default().into());
    s.set_install_dir(install::install_dir().display().to_string().into());
    if install::installed_version().is_some() {
        // an update keeps what the user chose
        s.set_autostart(config::autostart());
        s.set_desktop_shortcut(install::has_desktop_shortcut());
    }
    if let Some(name) = other_wallpaper_app() {
        s.set_warning(
            if tr.0 { format!("{name} läuft gerade. Beende es vor der Installation, damit sich beide Programme nicht stören.") } else { format!("{name} is running. Quit it before installing so the two programs do not get in each other's way.") }
                .into(),
        );
    }
    let weak = app.as_weak();

    let w2 = weak.clone();
    s.on_install(move || {
        let Some(app) = w2.upgrade() else { return };
        let s = app.global::<Setup>();
        s.set_page(2);
        s.set_step(0);
        let (autostart, desktop) = (s.get_autostart(), s.get_desktop_shortcut());
        let engine_was_running = crate::engine_window().is_some();
        let w3 = w2.clone();
        std::thread::spawn(move || {
            let result = install::install_with(autostart, desktop);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = w3.upgrade() else { return };
                let s = app.global::<Setup>();
                match result {
                    Ok(()) => {
                        // everything is done already; the list shows what happened
                        let step = Rc::new(std::cell::Cell::new(1));
                        let t = Rc::new(Timer::default());
                        let (w4, t2) = (w3.clone(), t.clone());
                        s.set_step(1);
                        t.start(TimerMode::Repeated, Duration::from_millis(160), move || {
                            let Some(app) = w4.upgrade() else { return };
                            let s = app.global::<Setup>();
                            step.set(step.get() + 1);
                            s.set_step(step.get());
                            if step.get() > 4 {
                                t2.stop();
                                // an update restarts through the running engine
                                if !engine_was_running {
                                    install::start_installed();
                                }
                                wait_for_engine(w4.clone());
                            }
                        });
                        std::mem::forget(t);
                    }
                    Err(e) => {
                        s.set_error_text(e.into());
                        s.set_page(3);
                    }
                }
            });
        });
    });

    s.on_open_panel(move || {
        if crate::engine_window().is_none() {
            install::start_installed();
        }
        install::open_installed_panel();
        let _ = slint::quit_event_loop();
    });
    run_setup_window(app, &cfg);
}

fn setup_window() -> (SetupWindow, Config) {
    let cfg = Config::load();
    let app = match SetupWindow::new() {
        Ok(a) => a,
        Err(e) => crate::fail(&format!("The setup could not open: {e}")),
    };
    apply_language(&cfg);
    apply_fonts(app.global::<crate::Theme>());
    app.invoke_set_dark(dark(&cfg));
    (app, cfg)
}

/// What the setup and the uninstaller share: the uninstall page and the way out.
fn run_setup_window(app: SetupWindow, cfg: &Config) {
    let tr = Tr(german(cfg));
    let s = app.global::<Setup>();
    s.set_data_size(size_text(install::data_size(), tr.0).into());
    let weak = app.as_weak();
    let de = tr.0;
    s.on_uninstall_now(move || {
        let Some(app) = weak.upgrade() else { return };
        let s = app.global::<Setup>();
        s.set_uninstalling(true);
        s.set_page(2);
        let delete_data = s.get_delete_data();
        let w2 = weak.clone();
        std::thread::spawn(move || {
            let ok = install::uninstall(delete_data);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = w2.upgrade() else { return };
                let s = app.global::<Setup>();
                if !ok {
                    s.set_error_text(Tr(de).t("Kumo konnte nicht entfernt werden. Schließe alle Kumo-Fenster und versuche es noch einmal.", "Kumo could not be removed. Close all Kumo windows and try again.").into());
                }
                s.set_page(if ok { 9 } else { 3 });
            });
        });
    });
    s.on_close(|| {
        let _ = slint::quit_event_loop();
    });
    title_bar_soon(app.as_weak(), dark(cfg));
    if let Some(p) = std::env::var("KUMO_UI_PAGE").ok().and_then(|v| v.parse::<i32>().ok()) {
        s.set_page(p);
    }
    let _ = app.run();
}

/// The window behind `kumo.exe --uninstall`, which is what Settings > Apps starts.
pub fn run_uninstall() {
    if ["Kumo deinstallieren", "Uninstall Kumo"].into_iter().any(focus_existing) {
        return;
    }
    let (app, cfg) = setup_window();
    let s = app.global::<Setup>();
    s.set_uninstalling(true);
    s.set_page(8);
    run_setup_window(app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(version_key("0.10.0") > version_key("0.2.0"));
        assert!(version_key("0.2.0") == version_key("0.2.0"));
        assert!(version_key("1.0.0") > version_key("0.9.9"));
    }

    #[test]
    fn lists_read_like_a_sentence() {
        let items: Vec<String> = ["1", "2", "3"].map(String::from).to_vec();
        assert_eq!(list_text(&items, &Tr(true)), "1, 2 und 3");
        assert_eq!(list_text(&items[..1], &Tr(false)), "1");
    }
}


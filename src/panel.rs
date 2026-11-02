//! Control panel (`kumo --panel`). It is a separate, short-lived process drawn by Slint's software
//! renderer, so it needs no GPU context and gives back everything on close. The panel edits
//! config.json and the library, then nudges the engine to reload. What differs per system is in
//! `os`; this file is shared with the Mac version.

use crate::config::{self, Config, Fit, Layout, PowerRule, TimeMode};
use crate::library::{self, Item, Kind};
use crate::{autostart, desktop, install, os, IdentifyWindow, PanelWindow, State};
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, Image, ModelRc, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

const LAPSE: [f32; 4] = [60.0, 240.0, 600.0, 2400.0];
const PETALS: [f32; 4] = [0.0, 0.5, 1.0, 1.6];
const FPS: [u32; 3] = [15, 30, 60];
const QUALITY: [f32; 3] = [1.0, 0.8, 0.62];
const SPEED: [f32; 4] = [0.5, 1.0, 1.5, 2.0];

// ---------------------------------------------------------------- language

pub(crate) fn german(cfg: &Config) -> bool {
    match cfg.language.as_str() {
        "de" => true,
        "en" => false,
        _ => os::system_german(),
    }
}

pub(crate) fn apply_language(cfg: &Config) {
    let _ = slint::select_bundled_translation(if german(cfg) { "de" } else { "en" });
}

/// Small strings built on the Rust side (the .slint files use @tr and lang/de).
pub(crate) struct Tr(pub bool);

impl Tr {
    pub(crate) fn t(&self, de: &'static str, en: &'static str) -> &'static str {
        if self.0 { de } else { en }
    }
}

pub(crate) fn dark(cfg: &Config) -> bool {
    match cfg.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => os::system_dark(),
    }
}

/// Dark or light system title bar to match the window. False while there is no native window yet.
fn title_bar(window: &slint::Window, dark: bool) -> bool {
    use winit::window::Theme;
    window.with_winit_window(|w| w.set_theme(Some(if dark { Theme::Dark } else { Theme::Light }))).is_some()
}

/// The native window only exists once the event loop runs.
pub(crate) fn title_bar_soon<T: ComponentHandle + 'static>(weak: slint::Weak<T>, dark: bool) {
    let timer = Rc::new(Timer::default());
    let t = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        if weak.upgrade().is_none_or(|app| title_bar(app.window(), dark)) {
            t.stop();
        }
    });
}

// ---------------------------------------------------------------- helpers

/// Opens a text that is built into the exe: Kumo is one file, with nothing next to it.
fn open_text(name: &str, text: &str) {
    let path = std::env::temp_dir().join(name);
    if std::fs::write(&path, text).is_ok() {
        os::open(&install::visible_path(&path).display().to_string(), None);
    }
}

pub(crate) fn size_text(bytes: u64, de: bool) -> String {
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

/// Wallpapers the engine could not show, with the reason.
fn failed_wallpapers() -> Vec<(String, String)> {
    std::fs::read_to_string(config::dir().join("status.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
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
    stats: os::EngineStats,
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
        os::notify_engine();
    }

    fn other_app(&mut self, fresh: bool) -> Option<&'static str> {
        if fresh || self.other_app_at.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            self.other_app = os::other_wallpaper_app();
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
    st.set_has_battery(os::has_battery());

    // settings
    st.set_autostart(autostart::autostart());
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
    st.set_library_path(install::visible_path(&library::dir()).display().to_string().into());
    st.set_packaged(install::packaged());
    st.set_mac(cfg!(target_os = "macos"));
    st.set_library_size(size_text(install::data_size(), tr.0).into());
    st.set_version(env!("CARGO_PKG_VERSION").into());

    if !ctx.detail_id.is_empty() {
        fill_detail(app, ctx);
    }
    show_info(app, ctx);
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
    let running = os::engine_running();
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
            if why.contains("decod") { os::codec_search(&tr) } else { "" },
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
                        crate::log(&format!("kumo: no thumbnail for {}: {e}", item.title));
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
pub fn run_panel() {
    if os::focus_existing("Kumo") {
        return;
    }
    library::sweep();
    let cfg = Config::load();
    let app = match PanelWindow::new() {
        Ok(a) => a,
        Err(e) => crate::fail(&format!("The control panel could not open: {e}")),
    };
    apply_language(&cfg);
    os::apply_fonts(app.global::<crate::Theme>());
    app.invoke_set_dark(dark(&cfg));
    let ctx = Rc::new(RefCell::new(Ctx {
        tr: Tr(german(&cfg)),
        cfg,
        thumbs: HashMap::new(),
        dropped: Vec::new(),
        info: Info::None,
        dismissed: Vec::new(),
        detail_id: String::new(),
        stats: os::EngineStats::default(),
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
        if !os::confirm(&title, body) {
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
            os::reveal(&install::visible_path(&if item.builtin { item.folder() } else { item.path() }));
        }
    });

    let (w2, c2) = (weak.clone(), ctx.clone());
    st.on_add_files(move || {
        let de = c2.borrow().tr.0;
        let files = os::pick_files(de);
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
            if st.get_autostart() != autostart::autostart() {
                autostart::set_autostart(st.get_autostart());
                // switched off in the system settings: only they can switch it back on
                if st.get_autostart() && !autostart::autostart() {
                    os::open_autostart_settings();
                }
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
            Info::EngineDown => os::start_engine(),
            Info::Failed(..) => os::open_codec_search(),
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

    st.on_open_log(|| os::open(&install::visible_path(&config::dir().join("kumo.log")).display().to_string(), None));
    st.on_open_library(|| {
        let _ = std::fs::create_dir_all(library::dir());
        os::open(&install::visible_path(&library::dir()).display().to_string(), None)
    });
    st.on_restart_engine(|| {
        os::stop_engine();
        os::start_engine();
    });
    st.on_quit_engine(os::stop_engine);
    let ch = change.clone();
    st.on_reset_all(move || {
        let de = german(&Config::load());
        let (t, b) = if de {
            ("Alle Einstellungen zurücksetzen?", "Bildrate, Qualität, Pausenregeln, Bildschirme und alle Anpassungen werden auf den Standard gesetzt, und die Pagode wird wieder dein Hintergrund. Deine Bibliothek bleibt erhalten.")
        } else {
            ("Reset all settings?", "Frame rate, quality, pause rules, screens and everything you customized go back to the defaults, and the pagoda becomes your wallpaper again. Your library stays.")
        };
        if os::confirm(t, b) {
            ch(&|_, c| {
                let (lang, theme) = (c.cfg.language.clone(), c.cfg.theme.clone());
                c.cfg = Config { language: lang, theme, ..Config::default() };
            });
        }
    });
    st.on_uninstall(os::uninstall);
    st.on_open_license(|| open_text("Kumo-License.txt", include_str!("../LICENSE")));
    st.on_open_notices(|| open_text("Kumo-Third-Party-Notices.txt", os::NOTICES));
    st.on_open_source(|| os::open("https://github.com/Hamido212/kumo", None));

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
                app.invoke_set_dark(os::system_dark());
                title_bar(app.window(), os::system_dark());
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
        st.set_panel_ram(format!("{:.0} MB", os::own_memory()).into());
        let was = st.get_running();
        show_info(&app, &mut c);
        if was != st.get_running() {
            refresh(&app, &mut c);
        }
    };
    tick();
    timer.start(TimerMode::Repeated, Duration::from_secs(2), tick);

    title_bar_soon(weak.clone(), dark(&ctx.borrow().cfg));

    if std::env::args().any(|a| a == "--welcome") {
        let w2 = weak.clone();
        Timer::single_shot(Duration::from_millis(500), move || {
            let de = german(&Config::load());
            let (t, b) = if de {
                (os::AUTOSTART_QUESTION.0, "Dein Live-Hintergrund ist dann nach jeder Anmeldung von selbst wieder da. Du kannst das jederzeit unter Einstellungen ändern.")
            } else {
                (os::AUTOSTART_QUESTION.1, "Your live wallpaper is then back by itself after every sign-in. You can change this at any time under Settings.")
            };
            if os::confirm(t, b) {
                autostart::set_autostart(true);
            }
            if let Some(app) = w2.upgrade() {
                app.global::<State>().set_autostart(autostart::autostart());
            }
        });
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_read_like_a_sentence() {
        let items: Vec<String> = ["1", "2", "3"].map(String::from).to_vec();
        assert_eq!(list_text(&items, &Tr(true)), "1, 2 und 3");
        assert_eq!(list_text(&items[..1], &Tr(false)), "1");
    }
}


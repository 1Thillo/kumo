//! User configuration (`%APPDATA%\Kumo\config.json`) and the "start with Windows" entry.
//!
//! The engine and the control panel share this file: the panel writes it and then tells the
//! running engine to reload (see `ipc` in main.rs).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use windows::core::{w, HSTRING, PCWSTR};
use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows::Win32::System::Registry::*;

pub const BUILTIN_SCENE: &str = "pagoda";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TimeMode {
    Real,
    Fixed,
    Lapse,
}

/// Settings of one 3D scene wallpaper.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub time: TimeMode,
    pub hour: f32,
    pub lapse: f32,
    pub rotate: f32,
    pub petals: f32,
    pub parallax: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { time: TimeMode::Real, hour: 16.5, lapse: 600.0, rotate: 1.0, petals: 1.0, parallax: false }
    }
}

impl Settings {
    /// Applies one `key=value` pair (command line overrides like `--hour 19`).
    pub fn set(&mut self, key: &str, v: &str) {
        let num = |d: f32| v.parse::<f32>().unwrap_or(d);
        match key {
            "time" => {
                self.time = match v {
                    "fixed" => TimeMode::Fixed,
                    "lapse" => TimeMode::Lapse,
                    _ => TimeMode::Real,
                }
            }
            "hour" => self.hour = num(16.5).rem_euclid(24.0),
            "lapse" => self.lapse = num(600.0).clamp(1.0, 100000.0),
            "rotate" => self.rotate = num(1.0).clamp(0.0, 10.0),
            "petals" => self.petals = num(1.0).clamp(0.0, 2.0),
            "parallax" => self.parallax = is_on(v),
            _ => {}
        }
    }
}

/// How a picture or video fills a screen of a different shape.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// fill the screen, crop the overhang
    Cover,
    /// show everything, bars where needed
    Contain,
    Stretch,
}

/// Settings of a video or image wallpaper.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct MediaSettings {
    pub fit: Fit,
    pub speed: f32,
}

impl Default for MediaSettings {
    fn default() -> Self {
        MediaSettings { fit: Fit::Cover, speed: 1.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// the same wallpaper on every screen
    Duplicate,
    /// each screen has its own wallpaper
    PerMonitor,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PowerRule {
    Keep,
    Half,
    Pause,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub fps: u32,
    /// render resolution relative to the screen
    pub quality: f32,
    pub fxaa: bool,
    pub layout: Layout,
    /// wallpaper id for every screen (duplicate) and for screens without an entry in `monitors`
    pub wallpaper: String,
    /// per-monitor mode: monitor id -> wallpaper id
    pub monitors: BTreeMap<String, String>,
    /// screens that show the normal Windows wallpaper instead of a live one
    pub off: Vec<String>,
    /// stop drawing a screen while a maximized or full-screen window covers it
    pub pause_when_covered: bool,
    pub on_battery: PowerRule,
    pub energy_saver: PowerRule,
    pub paused: bool,
    pub scenes: BTreeMap<String, Settings>,
    pub media: BTreeMap<String, MediaSettings>,
    /// "system", "light" or "dark"
    pub theme: String,
    /// "system", "de" or "en"
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            fps: 30,
            quality: 1.0,
            fxaa: true,
            layout: Layout::Duplicate,
            wallpaper: BUILTIN_SCENE.into(),
            monitors: BTreeMap::new(),
            off: Vec::new(),
            pause_when_covered: true,
            on_battery: PowerRule::Half,
            energy_saver: PowerRule::Half,
            paused: false,
            scenes: BTreeMap::new(),
            media: BTreeMap::new(),
            theme: "system".into(),
            language: "system".into(),
        }
    }
}

pub fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("Kumo")
}

pub fn config_path() -> PathBuf {
    dir().join("config.json")
}

fn is_on(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "true" | "yes" | "an" | "ja")
}

impl Config {
    pub fn load() -> Config {
        if let Ok(text) = std::fs::read_to_string(config_path()) {
            if let Ok(c) = serde_json::from_str::<Config>(&text) {
                return c.sanitized();
            }
        }
        Self::from_old_ini().unwrap_or_default()
    }

    /// Version 0.1 kept its settings in kumo.ini.
    fn from_old_ini() -> Option<Config> {
        let text = std::fs::read_to_string(dir().join("kumo.ini")).ok()?;
        let mut c = Config::default();
        let mut scene = Settings::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some((k, v)) = line.split_once('=') {
                c.set(k.trim(), v.trim());
                scene.set(k.trim(), v.trim());
            }
        }
        c.scenes.insert(BUILTIN_SCENE.into(), scene);
        Some(c)
    }

    fn sanitized(mut self) -> Config {
        self.fps = self.fps.clamp(5, 240);
        self.quality = self.quality.clamp(0.3, 2.0);
        if self.wallpaper.is_empty() {
            self.wallpaper = BUILTIN_SCENE.into();
        }
        self
    }

    /// Engine-wide `key=value` overrides (`--fps=60`, `--quality=0.8`, `--fxaa=off`).
    pub fn set(&mut self, key: &str, v: &str) {
        let num = |d: f32| v.parse::<f32>().unwrap_or(d);
        match key {
            "fps" => self.fps = (num(30.0) as u32).clamp(5, 240),
            "quality" | "scale" => self.quality = num(1.0).clamp(0.3, 2.0),
            "fxaa" => self.fxaa = is_on(v),
            _ => {}
        }
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(dir());
        if let Ok(text) = serde_json::to_string_pretty(self) {
            // write + rename, so the engine never reads a half-written file
            let tmp = config_path().with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, config_path());
            }
        }
    }

    pub fn scene(&self, id: &str) -> Settings {
        self.scenes.get(id).cloned().unwrap_or_default()
    }

    pub fn scene_mut(&mut self, id: &str) -> &mut Settings {
        self.scenes.entry(id.to_string()).or_default()
    }

    pub fn media(&self, id: &str) -> MediaSettings {
        self.media.get(id).cloned().unwrap_or_default()
    }

    /// Which wallpaper a monitor shows.
    pub fn wallpaper_for(&self, monitor_id: &str) -> &str {
        match self.layout {
            Layout::Duplicate => &self.wallpaper,
            Layout::PerMonitor => self.monitors.get(monitor_id).map(String::as_str).unwrap_or(&self.wallpaper),
        }
    }
}

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("Kumo");

/// The autostart of the Store package (package/AppxManifest.xml): its Run key would only reach
/// the package's private copy of the registry.
fn startup_task() -> Option<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from("KumoStartup")).ok()?.get().ok()
}

pub fn autostart() -> bool {
    if crate::install::packaged() {
        return startup_task().and_then(|t| t.State().ok()).is_some_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy);
    }
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, RRF_RT_REG_SZ, None, None, None).is_ok() }
}

pub fn set_autostart(on: bool) {
    if crate::install::packaged() {
        if let Some(task) = startup_task() {
            if on {
                let _ = task.RequestEnableAsync().and_then(|op| op.get());
            } else {
                let _ = task.Disable();
            }
        }
        return;
    }
    set_autostart_for(on, &std::env::current_exe().unwrap_or_default());
}

pub fn set_autostart_for(on: bool, exe: &std::path::Path) {
    unsafe {
        if on {
            let cmd = HSTRING::from(format!("\"{}\"", exe.display()));
            let bytes = (cmd.len() + 1) * 2;
            let _ = RegSetKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, REG_SZ.0, Some(cmd.as_ptr().cast()), bytes as u32);
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_or_partial_files_fall_back_to_defaults() {
        let c: Config = serde_json::from_str(r#"{"fps": 60}"#).unwrap();
        assert_eq!(c.fps, 60);
        assert_eq!(c.wallpaper, BUILTIN_SCENE);
        assert!(c.pause_when_covered);
        let c = serde_json::from_str::<Config>(r#"{"fps": 9999, "quality": 50, "wallpaper": ""}"#).unwrap().sanitized();
        assert_eq!(c.fps, 240);
        assert_eq!(c.quality, 2.0);
        assert_eq!(c.wallpaper, BUILTIN_SCENE);
    }

    #[test]
    fn wallpaper_per_screen() {
        let mut c = Config { wallpaper: "a".into(), ..Config::default() };
        c.monitors.insert("left".into(), "b".into());
        assert_eq!(c.wallpaper_for("left"), "a"); // duplicate mode ignores the map
        c.layout = Layout::PerMonitor;
        assert_eq!(c.wallpaper_for("left"), "b");
        assert_eq!(c.wallpaper_for("new screen"), "a");
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let mut c = Config::default();
        c.scene_mut("x").petals = 0.5;
        c.scene_mut("x").time = TimeMode::Lapse;
        c.on_battery = PowerRule::Pause;
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.scene("x").petals, 0.5);
        assert_eq!(back.scene("x").time, TimeMode::Lapse);
        assert_eq!(back.on_battery, PowerRule::Pause);
    }
}

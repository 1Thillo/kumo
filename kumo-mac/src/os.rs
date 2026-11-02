//! What the control panel needs from macOS. The same functions exist for Windows in src/os.rs, so
//! panel.rs itself does not care where it runs.

use crate::config;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSApplicationActivationOptions, NSModalResponseOK, NSOpenPanel, NSRunningApplication};
use objc2_foundation::{NSArray, NSLocale, NSString, NSUserDefaults};
use std::ffi::c_void;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const AUTOSTART_QUESTION: (&str, &str) = ("Kumo beim Anmelden starten?", "Start Kumo at login?");
pub const NOTICES: &str = include_str!("../assets/third-party-notices.txt");

const FILE_TYPES: [&str; 16] = ["mp4", "m4v", "mov", "webm", "mkv", "avi", "jpg", "jpeg", "png", "bmp", "webp", "tif", "tiff", "heic", "avif", "kumo"];
const ICONS: &str = "Kumo Icons";
const UI_FONT: &str = "Helvetica Neue";
/// How long a quitting engine gets before the panel carries on.
const STOP_WAIT: Duration = Duration::from_secs(5);

pub fn system_german() -> bool {
    NSLocale::preferredLanguages().iter().next().is_some_and(|l| l.to_string().starts_with("de"))
}

pub fn system_dark() -> bool {
    let key = NSString::from_str("AppleInterfaceStyle");
    NSUserDefaults::standardUserDefaults().stringForKey(&key).is_some_and(|s| s.to_string() == "Dark")
}

pub fn apply_fonts(theme: crate::Theme) {
    theme.set_icon_font(ICONS.into());
    theme.set_ui_font(UI_FONT.into());
}

pub fn open(target: &str, _args: Option<&str>) {
    let _ = Command::new("/usr/bin/open").arg(target).spawn();
}

pub fn confirm(title: &str, text: &str) -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(text));
    let (yes, no) = if system_german() { ("Ja", "Nein") } else { ("Yes", "No") };
    alert.addButtonWithTitle(&NSString::from_str(yes));
    alert.addButtonWithTitle(&NSString::from_str(no));
    alert.runModal() == NSAlertFirstButtonReturn
}

/// Wallpaper programs of other makers do not get in Kumo's way the way they do on Windows.
pub fn other_wallpaper_app() -> Option<&'static str> {
    None
}

// ---------------------------------------------------------------- one engine, one panel

/// The lock that says a process of this kind runs; the system frees it when the process ends.
pub fn try_lock(name: &str) -> Option<File> {
    let _ = std::fs::create_dir_all(config::dir());
    let file = File::create(config::dir().join(name)).ok()?;
    (unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(file)
}

pub const ENGINE_LOCK: &str = "kumo.lock";
const ENGINE_PID: &str = "engine.pid";
const PANEL_PID: &str = "panel.pid";

fn write_pid(name: &str) {
    let _ = std::fs::write(config::dir().join(name), std::process::id().to_string());
}

fn read_pid(name: &str) -> Option<i32> {
    std::fs::read_to_string(config::dir().join(name)).ok()?.trim().parse().ok()
}

/// Called once by the engine, after it holds the lock.
pub fn engine_started() {
    write_pid(ENGINE_PID);
}

pub fn engine_running() -> bool {
    try_lock(ENGINE_LOCK).is_none()
}

pub fn notify_engine() {
    if let Some(pid) = read_pid(ENGINE_PID).filter(|_| engine_running()) {
        unsafe { libc::kill(pid, libc::SIGUSR1) };
    }
}

fn spawn_self(args: &[&str]) {
    let Ok(exe) = std::env::current_exe() else { return };
    let _ = Command::new(exe).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}

pub fn start_engine() {
    spawn_self(&[]);
}

/// Asks the engine to quit and waits for it.
pub fn stop_engine() {
    if let Some(pid) = read_pid(ENGINE_PID).filter(|_| engine_running()) {
        unsafe { libc::kill(pid, libc::SIGTERM) };
        let started = Instant::now();
        while engine_running() && started.elapsed() < STOP_WAIT {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

pub fn open_panel(welcome: bool) {
    spawn_self(if welcome { &["--panel", "--welcome"] } else { &["--panel"] });
}

/// A second panel start brings the open one to the front instead.
pub fn focus_existing(_title: &str) -> bool {
    match try_lock("panel.lock") {
        Some(lock) => {
            std::mem::forget(lock);
            write_pid(PANEL_PID);
            false
        }
        None => {
            if let Some(app) = read_pid(PANEL_PID).and_then(NSRunningApplication::runningApplicationWithProcessIdentifier) {
                app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
            }
            true
        }
    }
}

// ---------------------------------------------------------------- numbers

/// `rusage_info_v0` from <sys/resource.h>.
#[repr(C)]
#[derive(Default)]
struct RusageInfo {
    uuid: [u8; 16],
    user_time: u64,
    system_time: u64,
    pkg_idle_wkups: u64,
    interrupt_wkups: u64,
    pageins: u64,
    wired_size: u64,
    resident_size: u64,
    phys_footprint: u64,
    proc_start_abstime: u64,
    proc_exit_abstime: u64,
}

#[repr(C)]
#[derive(Default)]
struct Timebase {
    numer: u32,
    denom: u32,
}

extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut c_void) -> i32;
    fn mach_timebase_info(info: *mut Timebase) -> i32;
}

const RUSAGE_INFO_V0: i32 = 0;
const MB: f64 = 1024.0 * 1024.0;

fn usage(pid: i32) -> Option<RusageInfo> {
    let mut info = RusageInfo::default();
    (unsafe { proc_pid_rusage(pid, RUSAGE_INFO_V0, (&mut info as *mut RusageInfo).cast()) } == 0).then_some(info)
}

fn seconds_per_tick() -> f64 {
    let mut t = Timebase::default();
    unsafe { mach_timebase_info(&mut t) };
    t.numer as f64 / t.denom.max(1) as f64 / 1e9
}

#[derive(Default)]
pub struct EngineStats {
    pid: i32,
    last: Option<(Instant, f64)>,
}

impl EngineStats {
    /// (memory MB, CPU % of the Mac) of the running engine: the "Memory" of the Activity Monitor.
    pub fn sample(&mut self) -> Option<(f64, f64)> {
        let pid = read_pid(ENGINE_PID).filter(|_| engine_running())?;
        if pid != self.pid {
            self.pid = pid;
            self.last = None;
        }
        let u = usage(pid)?;
        let now = (Instant::now(), (u.user_time + u.system_time) as f64 * seconds_per_tick());
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        let cpu = self.last.map(|(t, s)| (now.1 - s) / now.0.duration_since(t).as_secs_f64().max(0.001) / cores * 100.0).unwrap_or(0.0);
        self.last = Some(now);
        Some((u.phys_footprint as f64 / MB, cpu))
    }
}

pub fn own_memory() -> f64 {
    usage(std::process::id() as i32).map_or(0.0, |u| u.phys_footprint as f64 / MB)
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> *const c_void;
    fn IOPSCopyPowerSourcesList(blob: *const c_void) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFRelease(object: *const c_void);
}

pub fn has_battery() -> bool {
    unsafe {
        let blob = IOPSCopyPowerSourcesInfo();
        if blob.is_null() {
            return false;
        }
        let list = IOPSCopyPowerSourcesList(blob);
        let found = !list.is_null() && CFArrayGetCount(list) > 0;
        if !list.is_null() {
            CFRelease(list);
        }
        CFRelease(blob);
        found
    }
}

// ---------------------------------------------------------------- files

/// Native multi-select file picker.
pub fn pick_files(_de: bool) -> Vec<PathBuf> {
    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setAllowsMultipleSelection(true);
    panel.setCanChooseDirectories(false);
    let types: Vec<Retained<NSString>> = FILE_TYPES.iter().map(|t| NSString::from_str(t)).collect();
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&types)));
    if let Some(home) = std::env::var_os("HOME") {
        panel.setDirectoryURL(Some(&crate::media::ns_url(&Path::new(&home).join("Movies"))));
    }
    if panel.runModal() != NSModalResponseOK {
        return Vec::new();
    }
    panel.URLs().iter().filter_map(|u| u.path()).map(|p| PathBuf::from(p.to_string())).collect()
}

/// Shows a file in the Finder.
pub fn reveal(path: &Path) {
    let _ = Command::new("/usr/bin/open").arg("-R").arg(path).spawn();
}

pub fn open_autostart_settings() {
    open("x-apple.systempreferences:com.apple.LoginItems-Settings.extension", None);
}

/// macOS has no codecs to add, so a video it cannot play gets no button.
pub fn codec_search(_tr: &crate::panel::Tr) -> &'static str {
    ""
}

pub fn open_codec_search() {}

/// Deleting an app is the user's move on a Mac: this clears the way and shows them the app.
pub fn uninstall() {
    crate::autostart::set_autostart(false);
    if let Some(app) = crate::install::app_bundle() {
        reveal(&app);
    }
}

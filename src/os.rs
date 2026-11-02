//! What the control panel and the setup need from Windows. The Mac version has the same functions
//! in kumo-mac/src/os.rs, so panel.rs itself does not care where it runs.

use crate::{engine, install};
use std::path::{Path, PathBuf};
use std::time::Instant;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const AUTOSTART_QUESTION: (&str, &str) = ("Kumo mit Windows starten?", "Start Kumo with Windows?");
pub const NOTICES: &str = include_str!("../assets/third-party-notices.txt");

const OTHER_APPS: [(&str, &str); 3] = [("lively.exe", "Lively Wallpaper"), ("wallpaper32.exe", "Wallpaper Engine"), ("wallpaper64.exe", "Wallpaper Engine")];

pub fn system_german() -> bool {
    (unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff) == 0x07
}

pub fn system_dark() -> bool {
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

/// Windows 10 has neither of the two fonts the design uses.
pub fn apply_fonts(theme: crate::Theme) {
    use windows::Win32::UI::Shell::FOLDERID_Fonts;
    let fonts = install::known_folder(&FOLDERID_Fonts).unwrap_or_default();
    if !fonts.join("SegoeIcons.ttf").exists() {
        theme.set_icon_font("Segoe MDL2 Assets".into());
    }
    if !fonts.join("SegUIVar.ttf").exists() {
        theme.set_ui_font("Segoe UI".into());
    }
}

pub fn open(target: &str, args: Option<&str>) {
    let t = HSTRING::from(target);
    let a = args.map(HSTRING::from);
    unsafe {
        ShellExecuteW(None, w!("open"), &t, a.as_ref().map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr())), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

pub fn confirm(title: &str, text: &str) -> bool {
    unsafe { MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_YESNO | MB_ICONQUESTION | MB_SETFOREGROUND) == IDYES }
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

pub fn other_wallpaper_app() -> Option<&'static str> {
    let procs = running_processes();
    OTHER_APPS.iter().find(|(exe, _)| procs.iter().any(|p| p == exe)).map(|(_, name)| *name)
}

/// A second panel start brings the open one to the front instead.
pub fn focus_existing(title: &str) -> bool {
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

#[derive(Default)]
pub struct EngineStats {
    pid: u32,
    last: Option<(Instant, f64)>,
}

impl EngineStats {
    /// (memory MB, CPU % of the PC) of the running engine.
    pub fn sample(&mut self) -> Option<(f64, f64)> {
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

pub fn has_battery() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.BatteryFlag != 128 }
}

pub fn start_engine() {
    let exe = install::installed_exe();
    let exe = if exe.exists() { exe } else { std::env::current_exe().unwrap_or_default() };
    open(&exe.display().to_string(), None);
}

/// Asks the engine to quit and waits for it.
pub fn stop_engine() {
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

/// Native multi-select file picker.
pub fn pick_files(de: bool) -> Vec<PathBuf> {
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
        // a package starts in System32, so the first look should not be there
        if let Some(videos) = install::known_folder(&windows::Win32::UI::Shell::FOLDERID_Videos) {
            if let Ok(folder) = windows::Win32::UI::Shell::SHCreateItemFromParsingName::<_, _, windows::Win32::UI::Shell::IShellItem>(&HSTRING::from(videos.as_os_str()), None) {
                let _ = dlg.SetDefaultFolder(&folder);
            }
        }
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

pub fn engine_running() -> bool {
    crate::engine_window().is_some()
}

pub fn notify_engine() {
    crate::notify_engine();
}

pub fn own_memory() -> f64 {
    engine::memory().1
}

/// Shows a file in the Explorer.
pub fn reveal(path: &Path) {
    open("explorer.exe", Some(&format!("/select,\"{}\"", path.display())));
}

pub fn open_autostart_settings() {
    open("ms-settings:startupapps", None);
}

/// The button next to a video that could not be decoded.
pub fn codec_search(tr: &crate::panel::Tr) -> &'static str {
    tr.t("Im Store suchen", "Search the Store")
}

pub fn open_codec_search() {
    open("ms-windows-store://search/?query=video%20extensions", None);
}

pub fn uninstall() {
    let exe = install::installed_exe();
    if exe.exists() && !install::packaged() {
        open(&exe.display().to_string(), Some("--uninstall"));
    } else {
        open("ms-settings:appsfeatures", None);
    }
}

//! Self-installation for the signed-in user.
//!
//! `kumo.exe` copies itself to `%LOCALAPPDATA%\Programs\Kumo`, registers a Start menu entry, the
//! autostart entry and an entry in Settings → Apps, and can remove all of that again. Nothing here
//! needs administrator rights, so people without them can use Kumo too.
//!
//! The same exe also runs inside the Microsoft Store package, where Windows does all of this.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use windows::core::{w, Interface, GUID, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::Packaging::Appx::{GetCurrentPackageFamilyName, GetCurrentPackageFullName};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const UNINSTALL_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Kumo");

pub fn known_folder(id: &GUID) -> Option<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_CREATE, None).ok()?;
        let path = p.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(p.0 as *const _));
        path
    }
}

/// True inside the Microsoft Store package.
pub fn packaged() -> bool {
    static PACKAGED: OnceLock<bool> = OnceLock::new();
    *PACKAGED.get_or_init(|| {
        let mut len = 0;
        unsafe { GetCurrentPackageFullName(&mut len, PWSTR::null()) != APPMODEL_ERROR_NO_PACKAGE }
    })
}

/// Where a file in AppData really is. A package sees its private copy under the normal path, but
/// Explorer and the text editor are outside the package and only find it at the real one.
pub fn visible_path(path: &Path) -> PathBuf {
    if !packaged() {
        return path.to_path_buf();
    }
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    if unsafe { GetCurrentPackageFamilyName(&mut len, PWSTR(buf.as_mut_ptr())) } != ERROR_SUCCESS {
        return path.to_path_buf();
    }
    let family = String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize]);
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default());
    let private = local.join("Packages").join(family).join("LocalCache");
    let roaming = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default());
    [(&local, "Local"), (&roaming, "Roaming")]
        .iter()
        .find_map(|(base, name)| path.strip_prefix(base).ok().map(|rest| private.join(name).join(rest)))
        .filter(|real| real.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

pub fn install_dir() -> PathBuf {
    known_folder(&FOLDERID_UserProgramFiles).unwrap_or_else(|| PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Programs")).join("Kumo")
}

pub fn installed_exe() -> PathBuf {
    if packaged() {
        return std::env::current_exe().unwrap_or_default();
    }
    install_dir().join("kumo.exe")
}

/// True when this exe is the installed one; any other copy is a download and shows the setup.
pub fn is_installed_copy() -> bool {
    packaged() || std::env::current_exe().is_ok_and(|e| e.to_string_lossy().eq_ignore_ascii_case(&installed_exe().to_string_lossy()))
}

fn shortcut_path() -> PathBuf {
    known_folder(&FOLDERID_Programs).unwrap_or_default().join("Kumo.lnk")
}

fn desktop_shortcut_path() -> PathBuf {
    known_folder(&FOLDERID_Desktop).unwrap_or_default().join("Kumo.lnk")
}

pub fn has_desktop_shortcut() -> bool {
    desktop_shortcut_path().exists()
}

/// Every running Kumo (engine, panel, a downloaded setup) except this process and `except`.
pub fn running_copies(except: &[u32]) -> Vec<u32> {
    let mut pids = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return pids };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(0);
            let pid = e.th32ProcessID;
            if String::from_utf16_lossy(&e.szExeFile[..n]).to_ascii_lowercase().starts_with("kumo") && !except.contains(&pid) && pid != std::process::id() {
                pids.push(pid);
            }
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    pids
}

/// Asks every other running Kumo to quit, then makes sure it did.
fn close_running(except: &[u32]) {
    unsafe extern "system" fn cb(h: HWND, data: LPARAM) -> BOOL {
        let pids = &*(data.0 as *const Vec<u32>);
        let mut pid = 0;
        GetWindowThreadProcessId(h, Some(&mut pid));
        if pids.contains(&pid) {
            let _ = PostMessageW(h, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        true.into()
    }
    let pids = running_copies(except);
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&pids as *const _ as isize));
        for pid in pids {
            if let Ok(p) = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, false, pid) {
                if WaitForSingleObject(p, 4000) != WAIT_OBJECT_0 {
                    let _ = TerminateProcess(p, 1);
                    WaitForSingleObject(p, 2000);
                }
                let _ = CloseHandle(p);
            }
        }
    }
}

unsafe fn reg_set(key: HKEY, name: PCWSTR, value: &str) {
    let v = HSTRING::from(value);
    let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, (v.len() + 1) * 2);
    let _ = RegSetValueExW(key, name, 0, REG_SZ, Some(bytes));
}

unsafe fn reg_dword(key: HKEY, name: PCWSTR, value: u32) {
    let _ = RegSetValueExW(key, name, 0, REG_DWORD, Some(&value.to_le_bytes()));
}

fn create_shortcut(target: &Path, link: &Path) -> windows::core::Result<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let sl: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        sl.SetPath(&HSTRING::from(target.as_os_str()))?;
        sl.SetDescription(w!("Kumo live wallpaper"))?;
        sl.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)?;
        sl.cast::<IPersistFile>()?.Save(&HSTRING::from(link.as_os_str()), true)?;
    }
    Ok(())
}

pub fn installed_version() -> Option<String> {
    let mut buf = [0u16; 64];
    let mut len = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, UNINSTALL_KEY, w!("DisplayVersion"), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut len)).ok().ok()?;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(String::from_utf16_lossy(&buf[..n])).filter(|s| !s.is_empty())
}

/// Puts `image` in place of `dest`. A running exe cannot be overwritten but it can be renamed,
/// and on any failure the previous version goes back, so an update never leaves Kumo without an exe.
fn replace_exe(image: &[u8], dest: &Path) -> bool {
    let new = dest.with_extension("new.exe");
    if std::fs::write(&new, image).is_err() {
        let _ = std::fs::remove_file(&new);
        return false;
    }
    let mut old = dest.with_extension("old.exe");
    if dest.exists() {
        if old.exists() && std::fs::remove_file(&old).is_err() {
            old = dest.with_extension(format!("old.{}.exe", std::process::id())); // still running
        }
        if std::fs::rename(dest, &old).is_err() {
            let _ = std::fs::remove_file(&new);
            return false;
        }
    }
    if std::fs::rename(&new, dest).is_err() {
        let _ = std::fs::rename(&old, dest);
        return false;
    }
    true
}

/// Copies this exe into place and registers it. Returns 0 or an error number for the message.
///
/// If Kumo is running (an update), the engine is asked to restart itself from the new exe, and an
/// open control panel closes.
fn do_install(desktop: bool) -> u32 {
    let dir = install_dir();
    let dest = installed_exe();
    let Ok(src) = std::env::current_exe() else { return 2 };
    if std::fs::create_dir_all(&dir).is_err() {
        return 3;
    }
    let engine = crate::engine_window();
    // read and written, not copied: a copy would keep the download's "from the internet" mark
    if !src.to_string_lossy().eq_ignore_ascii_case(&dest.to_string_lossy()) {
        let Ok(image) = std::fs::read(&src) else { return 2 };
        if !replace_exe(&image, &dest) {
            return 4;
        }
    }
    let _ = std::fs::write(dir.join("LICENSE.txt"), include_str!("../LICENSE"));
    let _ = create_shortcut(&dest, &shortcut_path());
    if desktop {
        let _ = create_shortcut(&dest, &desktop_shortcut_path());
    } else {
        let _ = std::fs::remove_file(desktop_shortcut_path());
    }
    unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(HKEY_CURRENT_USER, UNINSTALL_KEY, 0, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut key, None).is_ok() {
            let exe = dest.display().to_string();
            reg_set(key, w!("DisplayName"), "Kumo Live Wallpaper");
            reg_set(key, w!("DisplayVersion"), env!("CARGO_PKG_VERSION"));
            reg_set(key, w!("Publisher"), "Kumo");
            reg_set(key, w!("DisplayIcon"), &exe);
            reg_set(key, w!("InstallLocation"), &dir.display().to_string());
            reg_set(key, w!("UninstallString"), &format!("\"{exe}\" --uninstall"));
            let size_kb = std::fs::metadata(&dest).map(|m| (m.len() / 1024) as u32).unwrap_or(0);
            reg_dword(key, w!("EstimatedSize"), size_kb);
            reg_dword(key, w!("NoModify"), 1);
            reg_dword(key, w!("NoRepair"), 1);
            let _ = RegCloseKey(key);
        }
    }
    // shortcuts keep showing a cached icon otherwise
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    let engine_pid = engine.map_or(0, |h| {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        pid
    });
    close_running(&[engine_pid]);
    if let Some(h) = engine {
        unsafe {
            let _ = PostMessageW(h, crate::WM_KUMO_RESTART, WPARAM(0), LPARAM(0));
            if let Ok(p) = OpenProcess(PROCESS_SYNCHRONIZE, false, engine_pid) {
                WaitForSingleObject(p, 3000);
                let _ = CloseHandle(p);
            }
        }
    }
    let _ = std::fs::remove_file(dir.join("kumo.old.exe"));
    0
}

/// Installs Kumo (the setup wizard runs this on a worker thread).
pub fn install_with(autostart: bool, desktop: bool) -> Result<(), String> {
    match do_install(desktop) {
        0 => {
            crate::config::set_autostart_for(autostart, &installed_exe());
            Ok(())
        }
        c if crate::tray::german() => Err(format!("Eine Datei konnte nicht geschrieben werden (Fehler {c}). Läuft Kumo noch?")),
        c => Err(format!("A file could not be written (error {c}). Is Kumo still running?")),
    }
}

/// Starts the installed engine.
pub fn start_installed() {
    let _ = std::process::Command::new(installed_exe()).current_dir(install_dir()).spawn();
}

/// Starts the installed control panel.
pub fn open_installed_panel() {
    let _ = std::process::Command::new(installed_exe()).arg("--panel").current_dir(install_dir()).spawn();
}

/// Command-line install (`kumo.exe --install`).
pub fn install_and_start() -> bool {
    let ok = install_with(true, false).is_ok();
    if ok {
        start_installed();
    }
    ok
}

/// The hidden command that removes the folder once the last Kumo process has gone. It keeps
/// trying for about five minutes, because the "removed" window may stay open that long.
fn cleanup_command(system: &str, dir: &str) -> String {
    format!("for /l %i in (1,1,300) do @(rd /s /q \"{dir}\" 2>nul & if not exist \"{dir}\" (exit) else \"{system}\\ping.exe\" -n 2 127.0.0.1 >nul)")
}

/// A running exe cannot delete itself, so a hidden command does it after this process has ended.
fn remove_after_exit(dir: &Path) {
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    let system = String::from_utf16_lossy(&buf[..n]);
    let cmd = HSTRING::from(format!("{system}\\cmd.exe"));
    let mut line: Vec<u16> = format!("\"{system}\\cmd.exe\" /c {}", cleanup_command(&system, &dir.display().to_string())).encode_utf16().chain([0]).collect();
    unsafe {
        let si = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut pi = PROCESS_INFORMATION::default();
        if CreateProcessW(&cmd, PWSTR(line.as_mut_ptr()), None, None, false, CREATE_NO_WINDOW, None, &HSTRING::from(system.as_str()), &si, &mut pi).is_ok() {
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
        }
    }
}

/// Removes Kumo. The Apps entry goes last: while anything can still fail, the user must be able
/// to run the uninstaller again.
fn do_uninstall() -> bool {
    close_running(&[]);
    crate::config::set_autostart(false);
    let _ = std::fs::remove_file(shortcut_path());
    let _ = std::fs::remove_file(desktop_shortcut_path());
    let dir = install_dir();
    let inside = std::env::current_exe().is_ok_and(|e| e.starts_with(&dir));
    for _ in 0..10 {
        if inside || !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    if dir.exists() {
        remove_after_exit(&dir);
    }
    let removed = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, UNINSTALL_KEY) };
    removed.is_ok() || removed == ERROR_FILE_NOT_FOUND
}

/// Size of a folder in bytes.
fn folder_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries.flatten().map(|e| if e.path().is_dir() { folder_size(&e.path()) } else { e.metadata().map_or(0, |m| m.len()) }).sum()
}

/// What "also delete my wallpapers" would free, in bytes.
pub fn data_size() -> u64 {
    folder_size(&crate::library::dir())
}

pub fn uninstall(delete_data: bool) -> bool {
    let ok = do_uninstall();
    if ok && delete_data {
        let lib = crate::library::dir();
        let _ = std::fs::remove_dir_all(lib.parent().unwrap_or(&lib));
        let _ = std::fs::remove_dir_all(crate::config::dir());
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Next to the test program, which is always writable.
    fn scratch(name: &str) -> PathBuf {
        std::env::current_exe().unwrap().with_file_name(format!("{name} {}", std::process::id()))
    }

    #[test]
    fn an_update_replaces_the_exe_and_keeps_the_old_one_on_failure() {
        let dir = scratch("update");
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("kumo.exe");
        assert!(replace_exe(b"one", &dest));
        assert_eq!(std::fs::read(&dest).unwrap(), b"one");
        // a folder in the way of the new file makes the update fail
        std::fs::create_dir(dir.join("kumo.new.exe")).unwrap();
        assert!(!replace_exe(b"two", &dest));
        assert_eq!(std::fs::read(&dest).unwrap(), b"one");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_cleanup_command_removes_the_folder() {
        let dir = scratch("cleanup");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub\\file.txt"), "x").unwrap();
        let mut buf = [0u16; 260];
        let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
        let system = String::from_utf16_lossy(&buf[..n]);
        let status = {
            use std::os::windows::process::CommandExt;
            std::process::Command::new(format!("{system}\\cmd.exe")).raw_arg(format!("/c {}", cleanup_command(&system, &dir.display().to_string()))).status().unwrap()
        };
        assert!(status.success());
        assert!(!dir.exists());
    }
}
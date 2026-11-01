//! Notification-area icon and its quick menu (German or English, following the Windows UI
//! language). Everything else lives in the control panel (`kumo.exe --panel`).

use crate::config::{Config, TimeMode};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAY: u32 = WM_APP + 1;

pub mod cmd {
    pub const OPEN: u32 = 2;
    pub const PAUSE: u32 = 1;
    pub const TIME: u32 = 10; // +0 real, +1 morning, +2 noon, +3 evening, +4 night, +5 time-lapse
    pub const FPS: u32 = 20; // +0 15, +1 30, +2 60
    pub const QUIT: u32 = 99;
}

pub const FIXED_HOURS: [f32; 4] = [7.5, 12.5, 18.6, 22.0];
pub const FPS_STEPS: [u32; 3] = [15, 30, 60];

struct Text {
    open: &'static str,
    pause: &'static str,
    resume: &'static str,
    time: &'static str,
    times: [&'static str; 6],
    fps: &'static str,
    quit: &'static str,
    paused: &'static str,
}

const DE: Text = Text {
    open: "Kumo öffnen",
    pause: "Pausieren",
    resume: "Fortsetzen",
    time: "Tageszeit (3D-Szenen)",
    times: ["Echtzeit (deine Uhr)", "Morgen", "Mittag", "Abendrot", "Nacht", "Zeitraffer"],
    fps: "Bildrate",
    quit: "Beenden",
    paused: "pausiert",
};

const EN: Text = Text {
    open: "Open Kumo",
    pause: "Pause",
    resume: "Resume",
    time: "Time of day (3D scenes)",
    times: ["Real time (your clock)", "Morning", "Noon", "Sunset", "Night", "Time-lapse"],
    fps: "Frame rate",
    quit: "Quit",
    paused: "paused",
};

pub fn german() -> bool {
    match Config::load().language.as_str() {
        "de" => true,
        "en" => false,
        _ => (unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff) == 0x07,
    }
}

fn text() -> &'static Text {
    if german() { &DE } else { &EN }
}


pub fn text_install_failed() -> &'static str {
    if german() { "Die Installation wurde abgebrochen oder ist fehlgeschlagen." } else { "Installation was cancelled or failed." }
}

pub fn text_no_output() -> &'static str {
    if german() {
        "Kumo konnte kein Hintergrundbild anzeigen und versucht es alle paar Sekunden erneut. Letzte Meldungen:"
    } else {
        "Kumo could not show the wallpaper and keeps retrying every few seconds. Recent messages:"
    }
}

pub fn text_attach_failed() -> &'static str {
    if german() {
        "Kumo konnte sich nicht hinter die Desktop-Symbole legen (Zugriff verweigert). Details stehen in %APPDATA%\\Kumo\\kumo.log."
    } else {
        "Kumo could not place itself behind the desktop icons (access denied). Details are in %APPDATA%\\Kumo\\kumo.log."
    }
}

/// A small cherry blossom, drawn procedurally so the exe needs no resources.
pub fn make_icon() -> HICON {
    const N: i32 = 32;
    let mut px = vec![0u32; (N * N) as usize];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5 - 16.0, y as f32 + 0.5 - 16.0);
            let mut petal = 0.0f32;
            for k in 0..5 {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 5.0;
                let (cx, cy) = (a.cos() * 8.0, a.sin() * 8.0);
                let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                petal = petal.max((7.6 - d).clamp(0.0, 1.0));
            }
            let center = (4.2 - (fx * fx + fy * fy).sqrt()).clamp(0.0, 1.0);
            let (r, g, b) = if center > 0.0 {
                let t = center;
                (255.0, 210.0 + 30.0 * t, 120.0 * (1.0 - t) + 210.0 * (1.0 - t))
            } else {
                (247.0, 168.0, 198.0)
            };
            let a = petal.max(center);
            px[(y * N + x) as usize] = ((a * 255.0) as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
        }
    }
    unsafe {
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: N,
                biHeight: -N,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(HDC::default(), &bi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0) else {
            return LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        };
        std::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u32, px.len());
        let mask_bits = vec![0u8; (N * N / 8) as usize];
        let mask = CreateBitmap(N, N, 1, 1, Some(mask_bits.as_ptr().cast()));
        let info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(color);
        let _ = DeleteObject(mask);
        icon
    }
}

pub struct Tray {
    nid: NOTIFYICONDATAW,
}

impl Tray {
    pub fn new(hwnd: HWND, icon: HICON, title: &str) -> Tray {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        set_tip(&mut nid, title);
        let t = Tray { nid };
        t.add();
        t
    }

    pub fn add(&self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &self.nid);
        }
    }

    pub fn set_tooltip(&mut self, s: &str) {
        set_tip(&mut self.nid, s);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &self.nid);
        }
    }

    pub fn remove(&self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
        }
    }
}

fn set_tip(nid: &mut NOTIFYICONDATAW, s: &str) {
    nid.szTip = [0; 128];
    for (i, c) in s.encode_utf16().take(127).enumerate() {
        nid.szTip[i] = c;
    }
}

pub fn tooltip(paused: bool) -> String {
    if paused { format!("Kumo ({})", text().paused) } else { "Kumo".to_string() }
}

unsafe fn item(menu: HMENU, id: u32, label: &str, checked: bool) {
    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
    let _ = AppendMenuW(menu, flags, id as usize, &HSTRING::from(label));
}

/// Shows the menu at the cursor and returns the chosen command (0 = none).
pub fn show_menu(hwnd: HWND, cfg: &Config) -> u32 {
    let t = text();
    let s = cfg.scene(&cfg.wallpaper);
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return 0 };
        let _ = AppendMenuW(menu, MF_STRING | MF_DEFAULT, cmd::OPEN as usize, &HSTRING::from(t.open));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        item(menu, cmd::PAUSE, if cfg.paused { t.resume } else { t.pause }, false);

        let sub = |label: &str, entries: &mut dyn Iterator<Item = (u32, &str, bool)>| {
            if let Ok(m) = CreatePopupMenu() {
                for (id, l, c) in entries {
                    item(m, id, l, c);
                }
                let _ = AppendMenuW(menu, MF_POPUP, m.0 as usize, &HSTRING::from(label));
            }
        };
        let hour_sel = |i: usize| s.time == TimeMode::Fixed && (s.hour - FIXED_HOURS[i]).abs() < 0.01;
        sub(t.time, &mut (0..6).map(|i| {
            let on = match i {
                0 => s.time == TimeMode::Real,
                5 => s.time == TimeMode::Lapse,
                _ => hour_sel(i - 1),
            };
            (cmd::TIME + i as u32, t.times[i], on)
        }));
        let fps_labels: Vec<String> = FPS_STEPS.iter().map(|f| format!("{f} fps")).collect();
        sub(t.fps, &mut (0..3).map(|i| (cmd::FPS + i as u32, fps_labels[i].as_str(), cfg.fps == FPS_STEPS[i])));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        item(menu, cmd::QUIT, t.quit, false);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, None);
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen.0 as u32
    }
}

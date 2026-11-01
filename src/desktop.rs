//! Monitors, the shell's desktop window, and the per-monitor wallpaper windows that sit
//! behind the desktop icons.
//!
//! Two shell layouts exist:
//! * Windows 11 24H2 and newer ("raised desktop"): Progman hosts SHELLDLL_DefView (icons) and
//!   a WorkerW (classic wallpaper). Our windows become children of Progman, z-ordered between
//!   the two, and have to be layered to show through the icon layer.
//! * Older Windows 10/11: a top-level WorkerW behind the icon window, our windows are its children.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Monitor {
    pub rect: RECT,
    /// stable id (the monitor's device path), used to remember which wallpaper it shows
    pub id: String,
    /// human readable name, e.g. "AOC 2702"
    pub name: String,
    pub primary: bool,
}

impl Monitor {
    pub fn w(&self) -> u32 {
        (self.rect.right - self.rect.left) as u32
    }
    pub fn h(&self) -> u32 {
        (self.rect.bottom - self.rect.top) as u32
    }
}

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

/// GDI device name ("\\.\DISPLAY1") -> (monitor device path, friendly name), via DisplayConfig.
fn display_names() -> Vec<(String, String, String)> {
    use windows::Win32::Devices::Display::*;
    let mut out = Vec::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm).is_err() {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None).is_err() {
            return out;
        }
        for p in &paths[..np as usize] {
            let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            src.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            src.header.adapterId = p.sourceInfo.adapterId;
            src.header.id = p.sourceInfo.id;
            let mut tgt = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            tgt.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            tgt.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            tgt.header.adapterId = p.targetInfo.adapterId;
            tgt.header.id = p.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut src.header) == 0 && DisplayConfigGetDeviceInfo(&mut tgt.header) == 0 {
                out.push((wide(&src.viewGdiDeviceName), wide(&tgt.monitorDevicePath), wide(&tgt.monitorFriendlyDeviceName)));
            }
        }
    }
    out
}

pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn cb(hm: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let list = &mut *(data.0 as *mut Vec<(Monitor, String)>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hm, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
            let m = Monitor { rect: mi.monitorInfo.rcMonitor, primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0, ..Default::default() };
            list.push((m, wide(&mi.szDevice)));
        }
        true.into()
    }
    let mut list: Vec<(Monitor, String)> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(HDC::default(), None, Some(cb), LPARAM(&mut list as *mut _ as isize));
    }
    let names = display_names();
    let mut all: Vec<Monitor> = list
        .into_iter()
        .enumerate()
        .map(|(i, (mut m, device))| {
            let found = names.iter().find(|(gdi, _, _)| gdi.eq_ignore_ascii_case(&device));
            m.id = found.map(|(_, path, _)| path.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| device.clone());
            m.name = found.map(|(_, _, n)| n.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Display {}", i + 1));
            m
        })
        .collect();
    // screen 1 is the main screen, the others follow from left to right
    all.sort_by_key(|m| (!m.primary, m.rect.left, m.rect.top));
    all
}

#[derive(Clone, Copy)]
pub struct Desktop {
    pub parent: HWND,
    pub raised: bool,
    defview: HWND,
    workerw: HWND,
}

fn find_child(parent: HWND, class: PCWSTR) -> Option<HWND> {
    unsafe { FindWindowExW(parent, None, class, None) }.ok().filter(|h| !h.is_invalid())
}

pub fn find() -> Option<Desktop> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), None).ok().filter(|h| !h.is_invalid())?;
        // ask the shell to create the WorkerW behind the icons (undocumented, used by every wallpaper engine)
        SendMessageTimeoutW(progman, 0x052C, WPARAM(0xD), LPARAM(1), SMTO_NORMAL, 1000, None);
        SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, None);

        if let (Some(defview), Some(workerw)) = (find_child(progman, w!("SHELLDLL_DefView")), find_child(progman, w!("WorkerW"))) {
            return Some(Desktop { parent: progman, raised: true, defview, workerw });
        }
        // classic layout: the WorkerW that follows the top-level window holding the icons
        unsafe extern "system" fn cb(h: HWND, data: LPARAM) -> BOOL {
            let found = &mut *(data.0 as *mut (bool, HWND));
            if find_child(h, w!("SHELLDLL_DefView")).is_some() {
                found.0 = true;
                if let Ok(next) = FindWindowExW(None, h, w!("WorkerW"), None) {
                    found.1 = next;
                }
            }
            true.into()
        }
        let mut found = (false, HWND::default());
        let _ = EnumWindows(Some(cb), LPARAM(&mut found as *mut _ as isize));
        let (icons, workerw) = found;
        if !icons {
            return None; // Explorer is still starting: the engine tries again
        }
        if workerw.is_invalid() {
            // some builds keep the icons directly in Progman without a separate WorkerW
            return Some(Desktop { parent: progman, raised: false, defview: HWND::default(), workerw: HWND::default() });
        }
        Some(Desktop { parent: workerw, raised: false, defview: HWND::default(), workerw })
    }
}

/// Whenever Windows sets a wallpaper (in Settings, by a slideshow) it puts a new WorkerW above
/// our windows, which hides Kumo. Sends every WorkerW back to the bottom.
pub fn keep_order(d: &Desktop) {
    if !d.raised {
        return;
    }
    let mut after = HWND::default();
    unsafe {
        while let Ok(h) = FindWindowExW(d.parent, after, w!("WorkerW"), None) {
            if h.is_invalid() {
                break;
            }
            let next = FindWindowExW(d.parent, h, w!("WorkerW"), None);
            if GetWindow(h, GW_HWNDNEXT).is_ok_and(|n| !n.is_invalid()) {
                let _ = SetWindowPos(h, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            let Ok(next) = next else { break };
            after = h;
            if next.is_invalid() {
                break;
            }
        }
    }
}

/// Makes the shell paint the user's wallpaper again where our windows were. The raised desktop
/// keeps it below us anyway. The current path has to go back in: without one, Windows replaces
/// the user's wallpaper with its default.
pub fn restore_wallpaper(d: &Desktop) {
    if d.raised {
        return;
    }
    let mut path = [0u16; 512];
    let none = SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0);
    unsafe {
        if SystemParametersInfoW(SPI_GETDESKWALLPAPER, path.len() as u32, Some(path.as_mut_ptr().cast()), none).is_ok() {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(path.as_mut_ptr().cast()), none);
        }
    }
}

/// Converts a screen rectangle into the parent's client coordinates.
fn to_parent(parent: HWND, r: RECT) -> RECT {
    let mut pts = [POINT { x: r.left, y: r.top }, POINT { x: r.right, y: r.bottom }];
    unsafe { MapWindowPoints(HWND::default(), parent, &mut pts) };
    RECT { left: pts[0].x, top: pts[0].y, right: pts[1].x, bottom: pts[1].y }
}

pub const CLASS: PCWSTR = w!("KumoWallpaper");

/// Creates the wallpaper window for one monitor. Returns (container, render window); the
/// swap chain goes on the render window.
pub fn create(d: &Desktop, m: &Monitor, instance: HINSTANCE) -> windows::core::Result<(HWND, HWND)> {
    let r = to_parent(d.parent, m.rect);
    crate::log(&format!(
        "kumo: monitor {:?} -> {} desktop, window at {:?}",
        m.rect, if d.raised { "raised (24H2+)" } else { "classic" }, r
    ));
    let style = WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN;
    let ex = WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
    unsafe {
        // per-monitor v2, otherwise Windows bitmap-scales the wallpaper on screens above 100 %
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let made = create_in(d, m, r, style, ex, instance);
        SetThreadDpiAwarenessContext(previous);
        made
    }
}

unsafe fn create_in(d: &Desktop, m: &Monitor, r: RECT, style: WINDOW_STYLE, ex: WINDOW_EX_STYLE, instance: HINSTANCE) -> windows::core::Result<(HWND, HWND)> {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    {
        // It starts as a hidden popup and is re-parented afterwards, like other wallpaper engines
        // do. Windows refuses this for a process below medium integrity.
        let container = CreateWindowExW(ex, CLASS, w!("Kumo"), WS_POPUP | WS_CLIPCHILDREN, m.rect.left, m.rect.top, w, h, None, None, instance, None)
            .inspect_err(|e| crate::log(&format!("kumo: container window: {}", e.message())))?;
        if let Err(e) = SetParent(container, d.parent) {
            crate::log(&format!("kumo: attaching to the desktop failed: {}", e.message()));
            let _ = DestroyWindow(container);
            return Err(e);
        }
        SetWindowLongW(container, GWL_STYLE, style.0 as i32);
        if d.raised {
            // like Lively: the raised desktop composes its children when they are layered
            SetWindowLongW(container, GWL_EXSTYLE, (ex | WS_EX_LAYERED).0 as i32);
            let _ = SetLayeredWindowAttributes(container, COLORREF(0), 255, LWA_ALPHA);
        }
        let _ = SetWindowPos(container, None, r.left, r.top, w, h, SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
        let inner = CreateWindowExW(WS_EX_NOACTIVATE, CLASS, w!("Kumo view"), style, 0, 0, w, h, container, None, instance, None)
            .inspect_err(|e| crate::log(&format!("kumo: view window: {}", e.message())))?;
        if d.raised {
            // icons stay on top, the classic wallpaper below us
            let _ = SetWindowPos(container, d.defview, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            // the classic wallpaper goes to the very bottom, below every monitor's window
            let _ = SetWindowPos(d.workerw, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        Ok((container, inner))
    }
}

/// A normal window for trying things out (`--window`).
pub fn create_test_window(w: i32, h: i32, instance: HINSTANCE) -> Option<HWND> {
    unsafe {
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE;
        let mut r = RECT { left: 0, top: 0, right: w, bottom: h };
        let _ = AdjustWindowRectEx(&mut r, style, false, WINDOW_EX_STYLE(0));
        CreateWindowExW(
            WINDOW_EX_STYLE(0), CLASS, w!("Kumo"), style, CW_USEDEFAULT, CW_USEDEFAULT,
            r.right - r.left, r.bottom - r.top, None, None, instance, None,
        )
        .ok()
    }
}

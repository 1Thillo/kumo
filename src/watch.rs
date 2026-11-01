//! Which monitors can actually see the wallpaper? A monitor counts as covered when a
//! maximized or full-screen window sits on it; Kumo stops drawing there.

use crate::desktop::Monitor;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

struct Ctx<'a> {
    monitors: &'a [Monitor],
    covered: Vec<bool>,
    own_pid: u32,
}

const IGNORED: [&str; 5] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "KumoWallpaper"];

unsafe extern "system" fn visit(h: HWND, data: LPARAM) -> BOOL {
    let ctx = &mut *(data.0 as *mut Ctx);
    if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
        return true.into();
    }
    let mut pid = 0;
    GetWindowThreadProcessId(h, Some(&mut pid));
    if pid == ctx.own_pid {
        return true.into();
    }
    let mut cloaked = 0u32;
    if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4).is_ok() && cloaked != 0 {
        return true.into(); // on another virtual desktop or hidden by the shell
    }
    let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TRANSPARENT.0 != 0 {
        return true.into();
    }
    let mut buf = [0u16; 64];
    let n = GetClassNameW(h, &mut buf) as usize;
    let class = String::from_utf16_lossy(&buf[..n]);
    if IGNORED.contains(&class.as_str()) {
        return true.into();
    }
    let mut r = RECT::default();
    if GetWindowRect(h, &mut r).is_err() {
        return true.into();
    }
    let zoomed = IsZoomed(h).as_bool();
    if !zoomed && ex & WS_EX_TOOLWINDOW.0 != 0 {
        return true.into();
    }
    let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
    for (i, m) in ctx.monitors.iter().enumerate() {
        let mr = m.rect;
        let inside = cx >= mr.left && cx < mr.right && cy >= mr.top && cy < mr.bottom;
        let fills = r.left <= mr.left && r.top <= mr.top && r.right >= mr.right && r.bottom >= mr.bottom;
        if (zoomed && inside) || fills {
            ctx.covered[i] = true;
        }
    }
    true.into()
}

static DIRTY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

unsafe extern "system" fn on_event(_: HWINEVENTHOOK, _: u32, h: HWND, obj: i32, child: i32, _: u32, _: u32) {
    // only top-level windows matter (not carets, cursors or controls inside apps)
    if obj == OBJID_WINDOW.0 && child == 0 && !h.is_invalid() && GetAncestor(h, GA_ROOT) == h {
        DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Lets Windows tell us when windows are moved, minimized, shown or closed, so a covered monitor
/// resumes as soon as the desktop is visible again. Each of these events wakes the engine, so
/// location changes (every mouse move) stay out; the 2 second check notices a maximize.
pub fn hook() {
    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    for (first, last) in [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND),
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
    ] {
        unsafe { SetWinEventHook(first, last, None, Some(on_event), 0, 0, flags) };
    }
}

/// True (once) when a window change was reported since the last call.
pub fn take_dirty() -> bool {
    DIRTY.swap(false, std::sync::atomic::Ordering::Relaxed)
}

pub fn covered(monitors: &[Monitor]) -> Vec<bool> {
    let mut ctx = Ctx { monitors, covered: vec![false; monitors.len()], own_pid: std::process::id() };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    ctx.covered
}

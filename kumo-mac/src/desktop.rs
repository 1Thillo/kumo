//! The screens of a Mac, in the shape the shared control panel expects.

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

#[derive(Clone)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone)]
pub struct Monitor {
    pub rect: Rect,
    pub id: String,
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

/// macOS counts from the bottom left of the main screen, in points. The first screen is the main one.
pub fn screen_id(screen: &NSScreen) -> String {
    screen.localizedName().to_string()
}

pub fn monitors() -> Vec<Monitor> {
    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    let screens = NSScreen::screens(mtm);
    screens
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let f = s.frame();
            let (left, top) = (f.origin.x as i32, -(f.origin.y + f.size.height) as i32);
            let rect = Rect { left, top, right: left + f.size.width as i32, bottom: top + f.size.height as i32 };
            Monitor { rect, id: screen_id(&s), name: screen_id(&s), primary: i == 0 }
        })
        .collect()
}

//! The menu bar item, the only part of Kumo a Mac user sees while the wallpaper runs.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSVariableStatusItemLength};
use objc2_foundation::{ns_string, NSData, NSLocale, NSSize, NSString};
use std::cell::{Cell, RefCell};

static ICON: &[u8] = include_bytes!("../../../assets/kumo-icon.png");

const ICON_SIZE: f64 = 18.0;

fn german() -> bool {
    NSLocale::preferredLanguages().iter().next().is_some_and(|l| l.to_string().starts_with("de"))
}

fn title(paused: bool) -> Retained<NSString> {
    let text = match (german(), paused) {
        (true, false) => "Pausieren",
        (true, true) => "Fortsetzen",
        (false, false) => "Pause",
        (false, true) => "Resume",
    };
    NSString::from_str(text)
}

struct Ivars {
    toggle: Cell<Option<fn() -> bool>>,
    item: RefCell<Option<Retained<NSMenuItem>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KumoMenuTarget"]
    #[ivars = Ivars]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(togglePause:))]
        fn toggle_pause(&self, _sender: Option<&AnyObject>) {
            if let Some(toggle) = self.ivars().toggle.get() {
                let paused = toggle();
                if let Some(item) = &*self.ivars().item.borrow() {
                    item.setTitle(&title(paused));
                }
            }
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl MenuTarget {
    fn new(toggle: fn() -> bool, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { toggle: Cell::new(Some(toggle)), item: RefCell::new(None) });
        unsafe { msg_send![super(this), init] }
    }
}

pub fn install(paused: bool, toggle: fn() -> bool, mtm: MainThreadMarker) {
    let target = MenuTarget::new(toggle, mtm);
    let menu = NSMenu::new(mtm);
    let pause = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &title(paused), Some(sel!(togglePause:)), ns_string!("")) };
    let quit_text = NSString::from_str(if german() { "Kumo beenden" } else { "Quit Kumo" });
    let quit = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &quit_text, Some(sel!(quit:)), ns_string!("q")) };
    unsafe {
        pause.setTarget(Some(&target));
        quit.setTarget(Some(&target));
    }
    menu.addItem(&pause);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&quit);
    target.ivars().item.replace(Some(pause));

    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = item.button(mtm) {
        let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(ICON));
        if let Some(image) = image {
            image.setSize(NSSize::new(ICON_SIZE, ICON_SIZE));
            button.setImage(Some(&image));
        } else {
            button.setTitle(ns_string!("Kumo"));
        }
    }
    item.setMenu(Some(&menu));
    // both live as long as the process: the status bar drops an item that is released
    std::mem::forget(target);
    std::mem::forget(item);
}

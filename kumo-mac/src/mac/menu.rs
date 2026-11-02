//! The menu bar item, the only part of Kumo a Mac user sees while the wallpaper runs.

use crate::os;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, sel, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSVariableStatusItemLength};
use objc2_foundation::{ns_string, NSData, NSSize, NSString};
use std::cell::{Cell, RefCell};

static ICON: &[u8] = include_bytes!("../../../assets/kumo-icon.png");

const ICON_SIZE: f64 = 18.0;

fn title(paused: bool) -> Retained<NSString> {
    let text = match (os::system_german(), paused) {
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

    unsafe impl NSObjectProtocol for MenuTarget {}

    unsafe impl NSApplicationDelegate for MenuTarget {
        // Kumo.app was opened again while it runs: there is no window to show but the panel
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _app: &NSApplication, _visible: bool) -> bool {
            os::open_panel(false);
            true
        }
    }

    impl MenuTarget {
        #[unsafe(method(openPanel:))]
        fn open_panel(&self, _sender: Option<&AnyObject>) {
            os::open_panel(false);
        }

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

pub fn install(app: &NSApplication, paused: bool, toggle: fn() -> bool, mtm: MainThreadMarker) {
    let target = MenuTarget::new(toggle, mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*target)));
    let de = os::system_german();
    let entry = |text: &NSString, action, key: &NSString| {
        let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), text, Some(action), key) };
        unsafe { item.setTarget(Some(&target)) };
        item
    };
    let menu = NSMenu::new(mtm);
    menu.addItem(&entry(&NSString::from_str(if de { "Kumo öffnen" } else { "Open Kumo" }), sel!(openPanel:), ns_string!("")));
    let pause = entry(&title(paused), sel!(togglePause:), ns_string!(""));
    menu.addItem(&pause);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&entry(&NSString::from_str(if de { "Kumo beenden" } else { "Quit Kumo" }), sel!(quit:), ns_string!("q")));
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

//! Setup wizard and uninstaller of the Windows version, drawn like the control panel.

use crate::config::Config;
use crate::panel::{apply_language, dark, german, size_text, title_bar_soon, Tr};
use crate::{autostart, install, os, Setup, SetupWindow};
use slint::{ComponentHandle, Timer, TimerMode};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Newest last, so "0.2.0" compares with "0.10.0" as a person would expect.
fn version_key(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Waits for the installed engine, then shows "Kumo is running".
fn wait_for_engine(weak: slint::Weak<SetupWindow>) {
    let started = Instant::now();
    let timer = Rc::new(Timer::default());
    let t = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        let Some(app) = weak.upgrade() else { return };
        let s = app.global::<Setup>();
        if crate::engine_window().is_some() {
            s.set_page(6);
            t.stop();
        } else if started.elapsed() > Duration::from_secs(10) {
            let de = german(&Config::load());
            s.set_error_text(Tr(de).t("Kumo ist installiert, aber nicht gestartet. Öffne es im Startmenü.", "Kumo is installed but did not start. Open it from the Start menu.").into());
            s.set_page(3);
            t.stop();
        }
    });
}

pub fn run_setup() {
    if ["Kumo installieren", "Install Kumo", "Kumo aktualisieren", "Update Kumo", "Kumo-Einrichtung", "Kumo setup", "Kumo deinstallieren", "Uninstall Kumo"].into_iter().any(os::focus_existing) {
        return;
    }
    let (app, cfg) = setup_window();
    let tr = Tr(german(&cfg));
    let s = app.global::<Setup>();
    s.set_version(env!("CARGO_PKG_VERSION").into());
    let installed = install::installed_version();
    if let Some(v) = &installed {
        // nothing to update: the page offers to open, repair or remove Kumo instead
        let own = version_key(env!("CARGO_PKG_VERSION"));
        if version_key(v) >= own {
            s.set_newer_installed(version_key(v) > own);
            s.set_page(7);
        }
    }
    s.set_installed_version(installed.unwrap_or_default().into());
    s.set_install_dir(install::install_dir().display().to_string().into());
    if install::installed_version().is_some() {
        // an update keeps what the user chose
        s.set_autostart(autostart::autostart());
        s.set_desktop_shortcut(install::has_desktop_shortcut());
    }
    if let Some(name) = os::other_wallpaper_app() {
        s.set_warning(
            if tr.0 { format!("{name} läuft gerade. Beende es vor der Installation, damit sich beide Programme nicht stören.") } else { format!("{name} is running. Quit it before installing so the two programs do not get in each other's way.") }
                .into(),
        );
    }
    let weak = app.as_weak();

    let w2 = weak.clone();
    s.on_install(move || {
        let Some(app) = w2.upgrade() else { return };
        let s = app.global::<Setup>();
        s.set_page(2);
        s.set_step(0);
        let (autostart, desktop) = (s.get_autostart(), s.get_desktop_shortcut());
        let engine_was_running = crate::engine_window().is_some();
        let w3 = w2.clone();
        std::thread::spawn(move || {
            let result = install::install_with(autostart, desktop);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = w3.upgrade() else { return };
                let s = app.global::<Setup>();
                match result {
                    Ok(()) => {
                        // everything is done already; the list shows what happened
                        let step = Rc::new(std::cell::Cell::new(1));
                        let t = Rc::new(Timer::default());
                        let (w4, t2) = (w3.clone(), t.clone());
                        s.set_step(1);
                        t.start(TimerMode::Repeated, Duration::from_millis(160), move || {
                            let Some(app) = w4.upgrade() else { return };
                            let s = app.global::<Setup>();
                            step.set(step.get() + 1);
                            s.set_step(step.get());
                            if step.get() > 4 {
                                t2.stop();
                                // an update restarts through the running engine
                                if !engine_was_running {
                                    install::start_installed();
                                }
                                wait_for_engine(w4.clone());
                            }
                        });
                        std::mem::forget(t);
                    }
                    Err(e) => {
                        s.set_error_text(e.into());
                        s.set_page(3);
                    }
                }
            });
        });
    });

    s.on_open_panel(move || {
        if crate::engine_window().is_none() {
            install::start_installed();
        }
        install::open_installed_panel();
        let _ = slint::quit_event_loop();
    });
    run_setup_window(app, &cfg);
}

fn setup_window() -> (SetupWindow, Config) {
    let cfg = Config::load();
    let app = match SetupWindow::new() {
        Ok(a) => a,
        Err(e) => crate::fail(&format!("The setup could not open: {e}")),
    };
    apply_language(&cfg);
    os::apply_fonts(app.global::<crate::Theme>());
    app.invoke_set_dark(dark(&cfg));
    (app, cfg)
}

/// What the setup and the uninstaller share: the uninstall page and the way out.
fn run_setup_window(app: SetupWindow, cfg: &Config) {
    let tr = Tr(german(cfg));
    let s = app.global::<Setup>();
    s.set_data_size(size_text(install::data_size(), tr.0).into());
    let weak = app.as_weak();
    let de = tr.0;
    s.on_uninstall_now(move || {
        let Some(app) = weak.upgrade() else { return };
        let s = app.global::<Setup>();
        s.set_uninstalling(true);
        s.set_page(2);
        let delete_data = s.get_delete_data();
        let w2 = weak.clone();
        std::thread::spawn(move || {
            let ok = install::uninstall(delete_data);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = w2.upgrade() else { return };
                let s = app.global::<Setup>();
                if !ok {
                    s.set_error_text(Tr(de).t("Kumo konnte nicht entfernt werden. Schließe alle Kumo-Fenster und versuche es noch einmal.", "Kumo could not be removed. Close all Kumo windows and try again.").into());
                }
                s.set_page(if ok { 9 } else { 3 });
            });
        });
    });
    s.on_close(|| {
        let _ = slint::quit_event_loop();
    });
    title_bar_soon(app.as_weak(), dark(cfg));
    if let Some(p) = std::env::var("KUMO_UI_PAGE").ok().and_then(|v| v.parse::<i32>().ok()) {
        s.set_page(p);
    }
    let _ = app.run();
}

/// The window behind `kumo.exe --uninstall`, which is what Settings > Apps starts.
pub fn run_uninstall() {
    if ["Kumo deinstallieren", "Uninstall Kumo"].into_iter().any(os::focus_existing) {
        return;
    }
    let (app, cfg) = setup_window();
    let s = app.global::<Setup>();
    s.set_uninstalling(true);
    s.set_page(8);
    run_setup_window(app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(version_key("0.10.0") > version_key("0.2.0"));
        assert!(version_key("0.2.0") == version_key("0.2.0"));
        assert!(version_key("1.0.0") > version_key("0.9.9"));
    }
}

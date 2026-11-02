//! The "start with Windows" entry: a Run key, or the StartupTask inside the Store package.

use windows::core::{w, HSTRING, PCWSTR};
use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows::Win32::System::Registry::*;

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("Kumo");

/// The autostart of the Store package (package/AppxManifest.xml): its Run key would only reach
/// the package's private copy of the registry.
fn startup_task() -> Option<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from("KumoStartup")).ok()?.get().ok()
}

pub fn autostart() -> bool {
    if crate::install::packaged() {
        return startup_task().and_then(|t| t.State().ok()).is_some_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy);
    }
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, RRF_RT_REG_SZ, None, None, None).is_ok() }
}

pub fn set_autostart(on: bool) {
    if crate::install::packaged() {
        if let Some(task) = startup_task() {
            if on {
                let _ = task.RequestEnableAsync().and_then(|op| op.get());
            } else {
                let _ = task.Disable();
            }
        }
        return;
    }
    set_autostart_for(on, &std::env::current_exe().unwrap_or_default());
}

pub fn set_autostart_for(on: bool, exe: &std::path::Path) {
    unsafe {
        if on {
            let cmd = HSTRING::from(format!("\"{}\"", exe.display()));
            let bytes = (cmd.len() + 1) * 2;
            let _ = RegSetKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, REG_SZ.0, Some(cmd.as_ptr().cast()), bytes as u32);
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE);
        }
    }
}

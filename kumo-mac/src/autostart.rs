//! Start at login through a user LaunchAgent: unlike SMAppService it works on every macOS version
//! Kumo runs on and does not need a Developer ID signature.

use std::path::PathBuf;

const LABEL: &str = "tech.hyos.kumo";

fn plist() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

pub fn autostart() -> bool {
    plist().is_some_and(|p| p.exists())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn set_autostart(on: bool) {
    let Some(path) = plist() else { return };
    if !on {
        let _ = std::fs::remove_file(path);
        return;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let text = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{LABEL}</string>
    <key>ProgramArguments</key><array><string>{}</string></array>
    <key>RunAtLoad</key><true/>
    <key>ProcessType</key><string>Interactive</string>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
"#,
        escape(&exe.display().to_string())
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, text);
}

/// The app may have been moved since the entry was written.
pub fn refresh() {
    if autostart() {
        set_autostart(true);
    }
}

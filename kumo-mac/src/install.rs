//! On a Mac the app is just dragged into Applications, so there is nothing to install. These are
//! the few answers the shared control panel asks for.

use std::path::{Path, PathBuf};

pub fn packaged() -> bool {
    false
}

pub fn visible_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn folder_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries.flatten().map(|e| if e.path().is_dir() { folder_size(&e.path()) } else { e.metadata().map_or(0, |m| m.len()) }).sum()
}

/// Everything Kumo keeps on disk apart from the app itself.
pub fn data_size() -> u64 {
    folder_size(&crate::config::dir())
}

/// `Kumo.app` around the running program.
pub fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

//! The wallpaper library: the built-in pagoda scene plus everything the user added.
//!
//! Every added wallpaper lives in its own folder `%APPDATA%\Kumo\library\<id>\` with the media
//! file, a `thumb.jpg` and a `wallpaper.json` describing it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub static PAGODA_SCENE: &[u8] = include_bytes!("../assets/pagoda.kumo");
static PAGODA_THUMB: &[u8] = include_bytes!("../assets/pagoda-thumb.jpg");

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Scene,
    Video,
    Image,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    /// media file name inside the item's folder (empty for the built-in scene)
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub builtin: bool,
}

pub fn dir() -> PathBuf {
    // big media files belong in the local (not roaming) profile
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(crate::config::dir);
    base.join("Kumo").join("library")
}

impl Item {
    pub fn folder(&self) -> PathBuf {
        dir().join(&self.id)
    }

    pub fn path(&self) -> PathBuf {
        self.folder().join(&self.file)
    }

    pub fn thumbnail(&self) -> PathBuf {
        self.folder().join("thumb.jpg")
    }

    /// The scene file's bytes (built-in or from disk).
    pub fn scene_bytes(&self) -> std::io::Result<std::borrow::Cow<'static, [u8]>> {
        if self.builtin {
            Ok(std::borrow::Cow::Borrowed(PAGODA_SCENE))
        } else {
            std::fs::read(self.path()).map(std::borrow::Cow::Owned)
        }
    }
}

fn builtin() -> Item {
    Item { id: crate::config::BUILTIN_SCENE.into(), title: "Voxel Pagoda Garden".into(), kind: Kind::Scene, file: String::new(), builtin: true }
}

/// All wallpapers: the built-in one first, then the user's, newest first.
pub fn list() -> Vec<Item> {
    let b = builtin();
    if !b.thumbnail().exists() {
        let _ = std::fs::create_dir_all(b.folder());
        let _ = std::fs::write(b.thumbnail(), PAGODA_THUMB);
    }
    let mut items = vec![b];
    let mut added: Vec<(std::time::SystemTime, Item)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for e in entries.flatten() {
            let meta = e.path().join("wallpaper.json");
            let Ok(text) = std::fs::read_to_string(&meta) else { continue };
            let Ok(item) = serde_json::from_str::<Item>(&text) else { continue };
            if item.builtin || item.id != e.file_name().to_string_lossy() {
                continue;
            }
            let when = std::fs::metadata(&meta).and_then(|m| m.created()).unwrap_or(std::time::UNIX_EPOCH);
            added.push((when, item));
        }
    }
    added.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    items.extend(added.into_iter().map(|(_, i)| i));
    items
}

pub fn get(id: &str) -> Option<Item> {
    list().into_iter().find(|i| i.id == id)
}

pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "kumo" => Some(Kind::Scene),
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "wmv" | "avi" => Some(Kind::Video),
        "jpg" | "jpeg" | "png" | "bmp" | "webp" | "tif" | "tiff" | "heic" | "avif" | "jxr" => Some(Kind::Image),
        _ => None,
    }
}

fn copy_with_progress(src: &Path, dst: &Path, progress: &mut impl FnMut(f32)) -> std::io::Result<()> {
    use std::io::{Read, Write};
    let mut from = std::fs::File::open(src)?;
    let total = from.metadata()?.len().max(1);
    let mut to = std::fs::File::create(dst)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0;
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        to.write_all(&buf[..n])?;
        done += n as u64;
        progress(done as f32 / total as f32);
    }
}

/// Copies a file into the library. The caller creates the thumbnail (`thumb.jpg`).
pub fn import(src: &Path, mut progress: impl FnMut(f32)) -> Result<Item, String> {
    let kind = kind_of(src).ok_or_else(|| format!("{}: unsupported file type", src.display()))?;
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "wallpaper".into());
    let slug: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).take(32).collect();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let id = format!("{}-{:x}", slug.trim_matches('-'), stamp);
    let ext = src.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let item = Item { id, title: stem, kind, file: format!("media.{ext}"), builtin: false };
    if kind == Kind::Scene {
        crate::scene::parse(&std::fs::read(src).map_err(|e| e.to_string())?)?;
    }
    std::fs::create_dir_all(item.folder()).map_err(|e| e.to_string())?;
    let copied = copy_with_progress(src, &item.path(), &mut progress).map_err(|e| e.to_string()).and_then(|_| save(&item));
    if copied.is_err() {
        let _ = std::fs::remove_dir_all(item.folder());
    }
    copied.map(|_| item)
}

/// Removes what an interrupted import left: folders without a `wallpaper.json`.
pub fn sweep() {
    for e in std::fs::read_dir(dir()).into_iter().flatten().flatten() {
        if e.file_name() != crate::config::BUILTIN_SCENE && !e.path().join("wallpaper.json").exists() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

pub fn save(item: &Item) -> Result<(), String> {
    let text = serde_json::to_string_pretty(item).map_err(|e| e.to_string())?;
    std::fs::write(item.folder().join("wallpaper.json"), text).map_err(|e| e.to_string())
}

pub fn remove(id: &str) -> Result<(), String> {
    let item = get(id).ok_or("unknown wallpaper")?;
    if item.builtin {
        return Err("the built-in scene cannot be removed".into());
    }
    // the engine lets go of a playing video only after it has reloaded
    let mut result = Ok(());
    for _ in 0..10 {
        result = std::fs::remove_dir_all(item.folder());
        if result.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_types() {
        assert_eq!(kind_of(Path::new("a.MP4")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("a.webm")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("b.jpeg")), Some(Kind::Image));
        assert_eq!(kind_of(Path::new("c.kumo")), Some(Kind::Scene));
        assert_eq!(kind_of(Path::new("d.txt")), None);
        assert_eq!(kind_of(Path::new("noextension")), None);
    }
}

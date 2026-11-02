//! Kumo for macOS. Scene format, simulation and settings are the files of the Windows version;
//! what differs is drawing (wgpu instead of Direct3D 11) and how a window gets behind the icons.

#[path = "../../src/config.rs"]
#[allow(dead_code)]
mod config;
#[path = "../../src/env.rs"]
#[allow(dead_code)]
mod env;
#[path = "../../src/library.rs"]
#[allow(dead_code)]
mod library;
#[path = "../../src/scene.rs"]
#[allow(dead_code)]
mod scene;
#[path = "../../src/world.rs"]
#[allow(dead_code)]
mod world;

mod gpu;
mod render;

#[cfg(target_os = "macos")]
mod autostart;
#[cfg(target_os = "macos")]
mod desktop;
#[cfg(target_os = "macos")]
mod install;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
mod media;
#[cfg(target_os = "macos")]
mod os;
#[cfg(target_os = "macos")]
#[path = "../../src/panel.rs"]
mod panel;

#[cfg(target_os = "macos")]
slint::include_modules!();

use config::{Config, Settings};

pub const SCENE_VIGNETTE: f32 = 0.28;

/// To stderr and into `kumo.log` next to the settings, which is what a tester can send back.
pub fn log(text: &str) {
    use std::io::Write;
    eprintln!("kumo: {text}");
    let _ = std::fs::create_dir_all(config::dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(config::dir().join("kumo.log")) {
        let _ = writeln!(f, "{text}");
    }
}

struct Args {
    shot: Option<String>,
    size: (u32, u32),
    warmup: f32,
    cfg: Config,
    settings: Settings,
}

fn parse_args() -> Args {
    let cfg = Config::load();
    let settings = cfg.scene(config::BUILTIN_SCENE);
    let mut a = Args { shot: None, size: (1280, 720), warmup: 6.0, cfg, settings };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--shot" => a.shot = it.next(),
            "--size" => a.size = it.next().and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))).unwrap_or(a.size),
            "--warmup" => a.warmup = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.warmup),
            "--hour" => {
                a.settings.set("time", "fixed");
                a.settings.set("hour", &it.next().unwrap_or_default());
            }
            other => {
                if let Some((k, v)) = other.strip_prefix("--").and_then(|o| o.split_once('=')) {
                    a.cfg.set(k, v);
                    a.settings.set(k, v);
                }
            }
        }
    }
    a
}

/// Ends the process with a message; only the panel gets here, when its window cannot open.
pub fn fail(message: &str) -> ! {
    log(message);
    std::process::exit(1);
}

/// `--import file`: adds a file to the library and makes it the wallpaper of every screen.
#[cfg(target_os = "macos")]
fn import(path: &str) -> Result<(), String> {
    let path = std::path::Path::new(path);
    let item = library::import(path, |_| {})?;
    let thumb = match item.kind {
        library::Kind::Image => media::thumbnail_from_file(&item.path(), &item.thumbnail()),
        _ => media::thumbnail_via_shell(&item.path(), &item.thumbnail()),
    };
    if let Err(e) = thumb {
        log(&format!("no thumbnail for {}: {e}", item.title));
    }
    let mut cfg = Config::load();
    cfg.wallpaper = item.id.clone();
    cfg.save();
    os::notify_engine();
    println!("imported {} as {}", path.display(), item.id);
    Ok(())
}

/// Renders one frame of the built-in scene offscreen and saves it (visual tests).
fn shot(path: &str, (w, h): (u32, u32), warmup: f32, cfg: &Config, s: &Settings) -> Result<(), String> {
    let gpu = gpu::Gpu::new()?;
    println!("{}", gpu.name());
    let post = render::Post::new(&gpu, gpu::COLOR);
    let scene = scene::parse(library::PAGODA_SCENE)?;
    let mut renderer = render::Renderer::new(&gpu, &scene, scene.manifest.shadow.size);
    let mut world = world::World::new(scene.manifest);
    for _ in 0..((warmup * 30.0) as usize).max(1) {
        world.update(1.0 / 30.0, s);
    }
    renderer.prepare(&gpu, &mut world);
    let src = gpu::Target::new(&gpu, ((w as f32 * cfg.quality) as u32).max(16), ((h as f32 * cfg.quality) as u32).max(16));
    let view = world.view(w as f32 / h as f32, false);
    renderer.render(&gpu, &world, &view, &src);
    let dst = gpu::Target::new(&gpu, w, h);
    post.draw(&gpu, &src, &dst.view, cfg.fxaa, SCENE_VIGNETTE);
    let rgba = dst.read_rgba(&gpu)?;
    write_bmp(path, w, h, &rgba).map_err(|e| e.to_string())?;
    println!("saved {path} ({w}x{h}, hour {:.2})", world.hour);
    Ok(())
}

fn write_bmp(path: &str, w: u32, h: u32, rgba: &[u8]) -> std::io::Result<()> {
    let row = (w * 3 + 3) & !3;
    let mut out = Vec::with_capacity((54 + row * h) as usize);
    let le = |v: u32| v.to_le_bytes();
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&le(54 + row * h));
    out.extend_from_slice(&le(0));
    out.extend_from_slice(&le(54));
    out.extend_from_slice(&le(40));
    out.extend_from_slice(&le(w));
    out.extend_from_slice(&le(h));
    out.extend_from_slice(&[1, 0, 24, 0]);
    out.extend_from_slice(&le(0));
    out.extend_from_slice(&le(row * h));
    for v in [2835u32, 2835, 0, 0] {
        out.extend_from_slice(&le(v));
    }
    for y in (0..h as usize).rev() {
        let line = &rgba[y * w as usize * 4..][..w as usize * 4];
        for p in line.chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        out.resize(out.len() + (row - w * 3) as usize, 0);
    }
    std::fs::write(path, out)
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--panel") {
            return panel::run_panel();
        }
        if let Some(at) = args.iter().position(|a| a == "--import") {
            if let Err(e) = import(args.get(at + 1).map_or("", String::as_str)) {
                eprintln!("kumo: {e}");
                std::process::exit(1);
            }
            return;
        }
    }
    let a = parse_args();
    if let Some(path) = &a.shot {
        if let Err(e) = shot(path, a.size, a.warmup, &a.cfg, &a.settings) {
            eprintln!("kumo: {e}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(target_os = "macos")]
    mac::run(a.cfg);
    #[cfg(not(target_os = "macos"))]
    eprintln!("kumo: the wallpaper itself only runs on a Mac here, use --shot");
}

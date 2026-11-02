# Kumo for macOS

Work in progress on the `kumo-mac` branch. The Windows version at the repository root is not
affected: this folder is its own Cargo workspace.

**Shared with Windows** (`#[path]` includes of `../src`): scene format, simulation, sky, settings,
the library and the control panel with its Slint UI and translations.

**Only here:** the renderer in wgpu (`src/render.rs`, `src/shaders.wgsl`, a port of the Direct3D 11
one, compared pixel by pixel against it) and the Mac layer in `src/mac/`: a borderless window per
screen, one level above the system wallpaper and below the desktop icons, and the menu bar item.

## What is built (so far only tried on a virtual macOS runner)

- The pagoda scene, own `.kumo` scenes, videos and pictures, per screen or on all of them.
- The control panel of the Windows version: `ui/` and `src/panel.rs` are shared, what differs per
  system is in `src/os.rs`.
- Pausing while a screen is covered, in Low Power Mode and on battery.
- Menu bar item, start at login through a LaunchAgent, thumbnails, a first-start welcome.

Videos play through AVPlayer in a Core Animation layer: decoding in hardware, no render loop.
macOS cannot play webm and most mkv files; such a video falls back to the built-in scene and the
panel says why.

## What is missing

A Developer ID signature and notarization, and tests on real Macs (`TESTEN.md`).

## Build

```bash
cargo run --release
```

`cargo run --release -- --shot shot.bmp --size 1920x1080 --hour 18.2` renders one frame offscreen.
That also works on Windows, where the wgpu renderer runs on Direct3D 12.

`.github/workflows/mac.yml` builds a universal `Kumo.app` for every push to this branch, renders
the scene and starts the app on a macOS runner.

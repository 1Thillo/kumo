# Kumo for macOS

Work in progress on the `kumo-mac` branch. The Windows version at the repository root is not
affected: this folder is its own Cargo workspace.

**Shared with Windows** (`#[path]` includes of `../src`): scene format, simulation, sky, settings
and the library.

**Only here:** the renderer in wgpu (`src/render.rs`, `src/shaders.wgsl`, a port of the Direct3D 11
one, compared pixel by pixel against it) and the Mac layer in `src/mac/`: a borderless window per
screen, one level above the system wallpaper and below the desktop icons, and the menu bar item.

## What is built (so far only tried on a virtual macOS runner)

- The built-in pagoda scene on every screen, behind the desktop icons and on all Spaces.
- Pausing while a screen is covered, in Low Power Mode and on battery (half frame rate by default).
- Menu bar item with pause and quit.

## What is missing

Video and image wallpapers, the control panel, start at login, a Developer ID signature.

## Build

```bash
cargo run --release
```

`cargo run --release -- --shot shot.bmp --size 1920x1080 --hour 18.2` renders one frame offscreen.
That also works on Windows, where the wgpu renderer runs on Direct3D 12.

`.github/workflows/mac.yml` builds a universal `Kumo.app` for every push to this branch, renders
the scene and starts the app on a macOS runner.

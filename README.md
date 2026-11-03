# Kumo ☁️ — a featherweight live wallpaper engine for Windows

Kumo (雲, "cloud") shows animated 3D scenes, videos and pictures behind your desktop icons, with
no browser and no .NET, in a single 19 MB program. It comes with a floating voxel island: a
five-storey pagoda, falling cherry blossoms, a koi pond, a waterfall and a real day and night
that follow your clock.

![Voxel Pagoda Garden](docs/preview.png)

Website: [kumo.hyos.tech](https://kumo.hyos.tech) · Deutsche Kurzanleitung: [weiter unten](#kurzanleitung-deutsch).

## Download

1. Download `Kumo-Setup-0.2.4.exe` from the [releases page](../../releases/latest).
2. Close Lively or Wallpaper Engine if they are running. Two wallpaper programs get in each
   other's way.
3. Double-click the file. Kumo is not signed yet, so Windows warns about it:
   * Your browser may call it an uncommon download. Choose **Keep**.
   * Windows shows "Windows protected your PC". Click **More info**, then **Run anyway**.

   The source code is in this repository and every release is built from it by
   [GitHub Actions](.github/workflows/build.yml), so you can check what you run.

   To be sure the file is the one GitHub built, check it before you start it. Every release has
   a `.sha256` file next to the exe:

   ```powershell
   (Get-FileHash .\Kumo-Setup-0.2.4.exe -Algorithm SHA256).Hash.ToLower()
   Get-Content .\Kumo-Setup-0.2.4.exe.sha256
   ```

   Both lines must start with the same value. Releases are also attested by GitHub, which shows
   that the file was built from this repository by the workflow above (needs the
   [GitHub CLI](https://cli.github.com)):

   ```powershell
   gh attestation verify .\Kumo-Setup-0.2.4.exe --repo Hamido212/kumo
   ```
4. Follow the three steps of the setup. Your new wallpaper starts right away.

You need Windows 10 or 11 (64-bit) and a graphics card with Direct3D 11, but no administrator
rights: Kumo installs into your own user folder.

## Why another wallpaper engine?

Most live wallpaper tools render through an embedded web browser, one complete browser per
screen. On a laptop with 4 to 8 GB of RAM that is a lot of memory for a background picture.

Measured on an Intel Iris Xe laptop:

| | Memory | Notes |
|---|---|---|
| Lively Wallpaper with the same scene, two screens (1440p + 1080p) | **about 900 MB** | Lively 135 MB plus one Edge WebView2 per screen |
| **Kumo, same two screens** | **about 74 MB** | GPU 3D load about 19 %, about 9 % of one CPU core |
| Kumo, 3D scene, one 1080p screen | about 63 MB | about 0.3 % of the whole CPU |
| Kumo, 1080p H.264 video | about 119 MB | hardware decoding, about 0.5 % of the whole CPU |
| Kumo, 1080p VP9 / AV1 video | about 86 / 91 MB | hardware decoding |
| Kumo, 4K HEVC video | about 213 MB | hardware decoding |
| Kumo, still picture | about 39 MB, then **0.3 MB** | memory goes back to Windows 30 s after drawing |
| Control panel (only while open) | about 8 MB | a separate process that ends when you close it |

Memory is the private working set, the "Memory" column in Task Manager. The graphics card holds
its own copy of textures on top of that.

### How it stays small

* **One process, one GPU device** for every screen. Meshes and shaders exist exactly once.
* **Shared frames:** screens with the same aspect ratio get one rendered frame, scaled to each.
* **Hardware video:** frames stay in the decoder's surfaces and go straight to the screen, and
  audio is never decoded. A video no screen can see costs nothing.
* **Pictures are drawn once:** then the texture is freed, because the screen keeps showing it.
* **Sleeps when nobody looks:** a screen covered by a maximized window is not drawn. If nothing
  needs drawing, or the PC is locked, the display is off or a Remote Desktop session runs, Kumo
  frees its render targets and waits with no CPU or GPU use. After 30 seconds it also frees its
  video decoders and hands its memory back to Windows.
* **The control panel is not part of the engine.** It is drawn in software by
  [Slint](https://slint.dev), needs no GPU context and exits when closed.
* **Baked scenes:** a world is generated once and stored as compact vertex data (20 bytes per
  vertex). Shaders are compiled when Kumo is built, so nothing is compiled at start-up.

## What it can do

* 3D scenes, videos (MP4, WebM, MKV and more, whatever Windows can decode) and pictures
* The same wallpaper everywhere, or a different one per screen, or none on a screen
* Add files by dragging them into the window
* Time of day for 3D scenes: your clock, a fixed time or time-lapse
* Fit (fill, fit, stretch) and playback speed for videos and pictures
* 15, 30 or 60 fps, render resolution, edge smoothing
* Pauses by itself: covered screens, battery and energy saver rules, locked PC, display off,
  Remote Desktop
* German and English, light and dark, usable with the keyboard
* Warns when another wallpaper program is running
* Stays on screen when Windows changes its own wallpaper, and falls back to the built-in scene
  with a clear message if a file cannot be shown

**Keyboard:** Tab moves between controls, Enter or Space activates, arrow keys change segments
and sliders, Esc goes back. `Ctrl+1`, `2`, `3` open Library, Screens and Performance, `Ctrl+,`
opens Settings, `Ctrl+O` adds files, and the Menu key opens a wallpaper's menu.

**Tray icon:** left click opens Kumo, right click shows pause, time of day, frame rate and quit.
Windows 11 may hide the icon behind the ^ arrow, and Kumo is always in the Start menu.

**Uninstall:** in *Windows Settings → Apps → Installed apps → Kumo Live Wallpaper*, or in Kumo
under *Settings → Uninstall Kumo*. You decide whether your wallpapers and settings go too. Your
normal Windows wallpaper comes back.

### Good to know

* **The program** is in `%LOCALAPPDATA%\Programs\Kumo`, so nothing needs administrator rights.
* **Wallpapers are copied** into `%LOCALAPPDATA%\Kumo\library`, so your originals can move.
* **Settings** are in `%APPDATA%\Kumo\config.json`, logs in `kumo.log` and `panel.log` next to it.
* **Animated GIFs are not supported yet.** Convert them to MP4.

### Command line

| Option | Effect |
|---|---|
| `--panel` | open the control panel |
| `--install`, `--uninstall` | install, or open the uninstall window |
| `--window [--size 1280x720]` | run in a normal window instead of behind the icons |
| `--hour 19` | fixed time of day |
| `--lapse 600` | time-lapse (600 = one day in 2.4 minutes) |
| `--fps=60`, `--quality=0.8`, `--rotate=0`, `--petals=1.5`, `--fxaa=off`, `--parallax=on` | override a setting |
| `--scene file.kumo` | with `--shot`: render another baked scene |
| `--shot out.bmp --size 2560x1440 [--hour 18.9] [--warmup 6]` | render one frame to a file and exit |
| `--stats`, `--log file` | write fps, frame time and memory to the log |

## Building

You need Rust (stable, MSVC toolchain) and the Visual Studio Build Tools.

```powershell
cargo test --profile fast      # tests with a quick build
cargo build --release          # target\release\kumo.exe, about 9 minutes
```

A program started from a folder with a Low integrity label, which some sandboxes set, cannot place
windows behind the desktop icons or reach a running Kumo: test with `--window` there.

`--profile fast` builds in about 3 minutes for day-to-day work. After changing a text in `ui/`,
run `python lang\make_po.py`, which also checks that every text has a German translation.

Scenes are baked from JavaScript, by the same generator that drives the browser preview:

```powershell
cd scenes\pagoda
npm install
npm run bake                   # assets\pagoda.kumo, then rebuild the exe
```

## Project layout

```
src/
  main.rs       entry point and modes (engine, --panel, --setup, --shot), window messages
  engine.rs     windows per screen, players per wallpaper, frame loop, pause rules
  video.rs      hardware video through Media Foundation and the GPU's video processor
  library.rs    wallpaper library, import and remove
  media.rs      pictures via WIC, JPEG thumbnails
  panel.rs      control panel, setup wizard, uninstall window
  install.rs    self-install for the user, Start menu, Apps entry, uninstall
  desktop.rs    screens, attaching windows behind the desktop icons (classic + 24H2 layout)
  watch.rs      which screens are covered by maximized windows
  gfx.rs        Direct3D 11 helpers
  render.rs     draw order: shadow map, voxels, petals, sky, water, particles, post pass
  world.rs      simulation: clock, camera, wind, petals, spray, fireflies, moving objects
  env.rs        time-of-day keyframes, sun and moon paths
  scene.rs      .kumo file format
  shaders.hlsl  all shaders (lighting matches three.js so scenes look like their preview)
  config.rs     config.json and autostart
  tray.rs       notification icon and menu
ui/             the Slint windows and the design tokens
lang/           German translation
scenes/pagoda/  scene generator (JavaScript) and bake script
assets/         baked scene, icon, pictures, third-party notices
docs/design/    UI specification and mockup
site/           the website (static files for Vercel, media rendered by tools/site_media.ps1)
package/        the Microsoft Store package: manifest, logos, listing pictures (see docs/store.md)
tools/          generators for the notices, the website media and the Store package
```

### The `.kumo` format

`"KUMO"`, version, a JSON manifest (camera, fog, shadow, sky keyframes, objects with optional
bob/orbit animation, swimmers, water, waterfall, petals, fireflies, glow halos, point lights)
and one binary blob with all vertices (`f32 x,y,z · i8 normal + flags · u8 sRGB colour + wind
sway`) followed by `u32` indices. See `scenes/pagoda/bake.mjs` for the writer and `src/scene.rs`
for the reader.

## Roadmap

* Signed releases, so the Windows warning goes away
* More scenes, and import from glTF (Blender) and MagicaVoxel
* Changing wallpapers on a timer, and rules for chosen apps
* Update check, GIF playback, optional video sound

## Kurzanleitung (Deutsch)

1. `Kumo-Setup-0.2.4.exe` von der [Release-Seite](../../releases/latest) herunterladen.
2. Lively oder Wallpaper Engine beenden, falls sie laufen.
3. Die Datei doppelklicken. Kumo ist noch nicht signiert, deshalb warnt Windows:
   * Im Browser **Behalten** wählen.
   * Bei „Der Computer wurde durch Windows geschützt“ auf **Weitere Informationen** und dann
     **Trotzdem ausführen** klicken.
4. Den drei Schritten im Assistenten folgen. Der neue Hintergrund startet sofort. Administratorrechte
   brauchst du nicht.

Danach findest du Kumo im Startmenü und unten rechts als Kirschblüte, die unter Windows 11 hinter
dem Pfeil ^ versteckt sein kann. Ein Linksklick öffnet Kumo, ein Rechtsklick zeigt Pausieren,
Tageszeit, Bildrate und Beenden. Videos und Bilder ziehst du einfach ins Fenster.

Deinstallieren: *Windows-Einstellungen → Apps → Installierte Apps → Kumo Live Wallpaper* oder in
Kumo unter *Einstellungen → Kumo deinstallieren*. Du entscheidest dort, ob deine Hintergründe und
Einstellungen mit gelöscht werden.

## License

Kumo is free software under the [GNU General Public License v3.0](LICENSE) (the same license
as Lively Wallpaper): you may use, study, change and share it, and changed versions you
distribute must be under the same license, with their source code. Its user interface is built
with [Slint](https://slint.dev), used under the GPL.

The authors and licenses of all libraries in Kumo are in
[assets/third-party-notices.txt](assets/third-party-notices.txt), which the program also shows
under *About Kumo*. After changing dependencies, run `python tools/third_party.py`.

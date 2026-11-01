# Kumo – Control panel & setup wizard UI specification

Status: draft for v0.2 · Owner: product/UX · Audience: the engineer who builds `kumo --panel` and the setup wizard
Companion file: [`mockup.html`](mockup.html). Open it in any browser on Windows (it uses the Segoe fonts). It has tabs for every screen, a dark/light switch, a DE/EN switch and "Alle zeigen". Deep links: `mockup.html#show=s-disp,theme=light,lang=en` (ids: `s-lib s-drop s-per s-detail s-disp s-perf s-set s-about w-1 w-2 w-3 w-4a w-4b w-5 w-x`). Dashed "v0.3" tags in the mockup mark Should features.

---

## 0. What this spec is for

Kumo exists for one reason: **live wallpapers that cost almost nothing**. Lively plus one web wallpaper on two screens used ~900 MB; Kumo plays the same 3D scene in ~74 MB. Every UI decision below is checked against that goal.

### Design principles

1. **The wallpaper is the product, the panel is a remote control.** The panel opens when needed and is fully closed afterwards. It never keeps running "for faster start".
2. **Cheapest path by default.** Same wallpaper on all screens, 30 fps, pause when covered. Anything that costs memory says so, in numbers.
3. **Show the cost.** Kumo's RAM use is always visible, in the sidebar and on the performance page. Choices that add memory (two different wallpapers, 4K video) show an estimate before you commit.
4. **Windows 11, not a skin.** Standard Fluent controls and spacing, Segoe UI Variable, one muted accent colour. No splash screens, no idle animations, no live previews.
5. **No Save buttons.** Every change applies straight away and is saved straight away (Windows Settings model). Anything destructive asks first.

---

## 1. Research summary

### 1.1 Lively Wallpaper (lively-community/lively, WinUI 3 + .NET)

Taken from the repository (`SettingsModel.cs`, `Strings/en-US/Resources.resw`, views), the wiki and reviews:

| Area | What Lively has |
|---|---|
| Wallpaper types | Video (mpv/VLC/WMF), GIF, picture, web page (WebView2/CefSharp), Shadertoy URL, web streams (YouTube), applications/games (Unity, Godot), AI "depth" wallpaper from a photo |
| Main window | NavigationView: **Library** (tile grid, search, "Add wallpaper", "…" menu: Set as wallpaper / Customise / Preview / Open file location / Save to file / Delete), Gallery (online, account), Settings, Help, About; "Control panel" button for screens |
| Add wallpaper | Drag & drop, "Choose a file", "Enter URL", "Advanced → Create wallpaper / Depth wallpaper" |
| Customise | Per-wallpaper property panel (sliders, colours, checkboxes, dropdowns defined by `LivelyProperties.json`) |
| Screens ("Choose Display") | Placement method: **Screen** (per screen) / **Span** / **Duplicate**; "Always pick screen when choosing wallpaper"; screensaver layout tab |
| Performance | Pause algorithm (Grid / All process / Foreground process / Direct3D), **Applications focused**, **Applications fullscreen**, **When on Battery Power**, **When on Battery Saver**, **When on Remote Desktop** (each: Pause / Kill / Nothing), **Display pause rule** (Per screen / All screens), **Application rules** (per .exe) |
| Wallpaper settings | Choose a fit (Fill / Uniform / Uniform Fill / None), wallpaper input (mouse, keyboard), video player + GPU decode, GIF player, web browser engine + disk cache, stream detection & quality |
| Audio | Global volume, "Play audio only when desktop is focused", audio output per screen, visualiser device, music app exclusion |
| General | Start with Windows, language, theme (System/Light/Dark + colour, dynamic theme from wallpaper), UI mode Normal/Lite, tile size, wallpaper directory, tray icon visibility, **Fast startup (keep UI running)** |
| System | Taskbar theme (Blur / Clear / Color / Fluent / Wallpaper…), desktop picture & lock screen picture from the live wallpaper, logs, debug menu, beta channel |
| Screensaver | Mirror wallpaper or custom, idle time, fade to black, lock on resume |
| Other | Playlists, command line, wallpaper API (hardware/audio/music data), welcome wizard, update check |

### 1.2 Wallpaper Engine (Steam, closed source)

| Area | What Wallpaper Engine has |
|---|---|
| Types | Scene (own 2D/3D engine with particles and effects), video, web, application |
| Content | Steam Workshop (huge catalogue, ratings, tags, filters), built-in editor |
| Customise | Per-wallpaper user properties, colour, playback speed, audio |
| Screens | Per monitor, clone, stretch across screens, layouts saved per monitor configuration |
| Playlists | Interval, time of day, day of week, on login; transitions; random or in order |
| Performance | FPS limit, quality presets (antialiasing, post-processing, texture size, reflections), "Other application focused / maximized / fullscreen → Keep running / Mute / Pause / Stop (free memory)", laptop on battery, application rules, Remote Desktop pause |
| Extras | Audio-reactive wallpapers, media integration, RGB/LED (iCUE, Chroma), screensaver, mobile app, lock screen |

### 1.3 What Kumo takes from this

* The **interaction model** both share: a library grid where one click applies, a customise panel driven by the wallpaper's own property list, a screen picker, pause rules per situation.
* WE's **"Stop (free memory)"** idea, built in as Kumo's *still image* mode.
* Lively's **"desktop picture"** idea (put a still of the live wallpaper behind it), so pausing, stopping and quitting look the same.
* Everything else is filtered through RAM, see §2.

---

## 2. Feature list for Kumo

RAM numbers marked "≈" are estimates for planning. Measure them before they appear in the UI.

### Must (v0.2)

| # | Feature | RAM / complexity note |
|---|---|---|
| M1 | **Control panel as an on-demand process** (`kumo.exe --panel`), opened from the tray, fully exits when closed | 0 MB when closed. Target ≤ 40 MB while open (hard limit 60). Medium |
| M2 | **Library**: built-in *Voxel Pagoda Garden* plus import by drag & drop or file picker (`.kumo`, `.mp4`, `.webm`, `.jpg`, `.png`, `.bmp`, `.webp`) | Static 16:9 JPEG thumbnails, decoded at display size, grid virtualised ≈ 5–10 MB in the panel. Medium |
| M3 | **Click to apply**; "same on all screens" or "per screen"; turn the live wallpaper off on a single screen | Identical wallpapers already share one frame. Each extra distinct 3D scene ≈ +15–30 MB. An "off" screen costs 0. Medium |
| M4 | **Displays page** with a monitor layout diagram and **Identify** (big numbers on each screen for 3 s) | 0 MB idle. The engine draws the numbers briefly. Medium |
| M5 | **Video wallpapers**: H.264 MP4 always; HEVC, VP9 and AV1 when Windows has the codec. Media Foundation hardware decode into D3D11, looped, muted | ≈ +20–40 MB per distinct video, decode surfaces live in GPU memory. One decoder per file, even on several screens. Large |
| M6 | **Image wallpapers set through Windows itself** (`IDesktopWallpaper::SetWallpaper` per monitor); Kumo's renderer sleeps on that screen | **0 MB**, the cheapest possible path. The original wallpaper is saved and restored. Small |
| M7 | **Customise page** generated from each wallpaper's property list (pagoda: time of day, camera, petals, parallax; video and image: fit) | 0 MB. Values go to the engine over the pipe and apply live. Medium |
| M8 | **Playback & performance**: 15/30/60 fps, render scale 100/80/62 %, FXAA, global pause, live RAM/GPU/CPU readout | Status message every 2 s, only while the page is visible. Small |
| M9 | **Pause rules**: windows covering a screen (maximized+fullscreen / fullscreen only / never), on battery, with energy saver (Normal / half frame rate / pause). Always paused when locked, display off, or in Remote Desktop | Saves RAM and power. The rules engine already exists. Small |
| M10 | **Settings**: start with Windows, language (System/Deutsch/English), colour mode (System/Light/Dark), copy imported files, open log or folder, reset | Small |
| M11 | **Setup wizard**: install, finish page (variant A: Start menu, variant B: launch via Explorer with automatic fallback), in-place update, repair, uninstall with "keep my data" | Runs only from the downloaded copy. Medium |
| M12 | **Slim tray menu**: left-click opens the panel; menu has Kumo öffnen, Pausieren, Bildrate, Beenden | Small |
| M13 | **Conflict detection**: Lively (`Lively.exe`) or Wallpaper Engine (`wallpaper32/64.exe`) running → warning InfoBar | Small |

### Should (v0.3)

| # | Feature | RAM / complexity note |
|---|---|---|
| S1 | Library search and type filter (All / 3D / Video / Image) | 0 MB. Small. Layout space is reserved in v0.2 |
| S2 | Video audio: "Play sound" plus volume; only audible while the desktop is visible | ≈ +2–5 MB (audio renderer) while enabled. Small |
| S3 | Rotation per screen: change every N minutes or by time of day, with crossfade | One extra render target only during the 1 s fade. Medium |
| S4 | App rules: pause (or still image) while chosen `.exe` files run | 0 MB. Small |
| S5 | "Play only on the empty desktop" (pause whenever another window has focus) | Saves power. Small |
| S6 | **Still image mode**: release the GPU device and show a matching still as the Windows wallpaper (on battery, on quit, by rule) | Engine drops to ≈ 10 MB. Medium (render still, set via Windows, restore original) |
| S7 | GIF import by **one-time conversion to MP4** (Media Foundation H.264 encoder). GIF frames are never decoded into RAM | A 1080p GIF in RAM can take hundreds of MB, so it plays as video. Medium |
| S8 | Manual "Check for updates" (GitHub Releases API via WinHTTP; opens the browser to download) | 0 MB idle. Small |
| S9 | Remember the panel's size and position; compact sidebar under 860 px width | Small |
| S10 | Pause scope: only the covered screen (default) or all screens | Small |
| S11 | Docked customise mode: the panel shrinks to a 360 px strip at the screen edge so the desktop stays visible | Small–medium |
| S12 | High-contrast theme support (system colours) | Medium |

### Later

| Feature | RAM / complexity note |
|---|---|
| **Web/HTML wallpapers** as an opt-in "compatibility" type | WebView2 ≈ +150–400 MB per instance. One instance per wallpaper, not per screen. Red RAM warning, stops on battery. Large |
| Span one wallpaper across all screens | Video first (one decode, crop per screen). 3D needs a wide camera. Medium |
| Screensaver (`.scr` shim that asks the running engine) | 0 MB extra. Medium |
| Lock screen picture | WinRT API may need package identity. Small, risky |
| Shader wallpapers (single-pass HLSL) | ~0 MB RAM but heavy on the GPU, capped at 30 fps. Medium |
| glTF and MagicaVoxel import to `.kumo` (roadmap) | Build-time tool, 0 MB at runtime. Large |
| Community scene catalogue (static index, no accounts) | Medium |
| Global hotkey for pause, export/share a wallpaper with its settings, mute when other audio plays, frame rate per wallpaper, changing the library location | Small each |
| Automatic update install | Needs code signing first. Medium |

### Never

| Feature | Why not |
|---|---|
| Applications/games (Unity, Godot, `.exe`) as wallpaper | Unbounded RAM, security risk |
| Online gallery with accounts, Steam Workshop, ratings | Servers, privacy, moderation |
| YouTube/stream URLs | Needs a browser or yt-dlp stack |
| AI depth wallpapers | ONNX runtime ≈ 100+ MB, download size |
| Always-on audio visualiser, RGB/LED integration, "now playing" integration | Constant capture/polling, CPU wake-ups |
| Taskbar theming, changing the Windows accent colour | System hacks, not a wallpaper's job |
| "Fast startup – keep UI running", panel minimising to the tray | Directly contradicts the RAM goal |
| Animated or hover video previews in the library | RAM and CPU while browsing |
| Pause-algorithm chooser, "Lite UI mode", tile size setting | Kumo should simply do the right thing |
| Built-in scene editor | Scenes are baked by external tools |
| Telemetry/analytics | Privacy. The only network call is the manual update check |
| Mobile companion app | Out of scope |

---

## 3. Architecture assumptions the UI depends on

| Topic | Decision |
|---|---|
| Processes | `kumo.exe` (engine + tray, always running) and `kumo.exe --panel` (the same exe, a separate process). One file is easier to distribute. UI code that is never executed in engine mode is never paged in, so the engine's private working set does not grow. |
| Opening | Tray left-click, tray "Kumo öffnen", or the Start menu "Kumo" entry when the engine is already running. The engine spawns the panel (a Program Files process, so it is trusted) and calls `AllowSetForegroundWindow`. Single instance: a second request brings the existing window to the front. |
| Closing | Window close = process exit. No minimise-to-tray. Pending writes are flushed on `WM_CLOSE`. |
| IPC | Named pipe `\\.\pipe\kumo-<user SID>`, one JSON object per line. The panel sends `set` / `apply` / `pause` / `identify`; the engine sends `state` (on change) and `stats` (every 2 s while the panel asks for them). The engine owns all state; the panel never writes `kumo.ini` itself. |
| Elevation | The panel must **never** run elevated: Windows blocks drag & drop from Explorer into elevated windows (UIPI). |
| Toolkit | Recommended: **Slint with the software renderer** and the Fluent style. It needs no GPU context, which saves ≈ 20–40 MB of driver DLLs compared with OpenGL/wgpu. egui is possible but needs a GPU backend. No WebView. |
| Idle behaviour | **No redraws while idle.** The only periodic update is the 2 s stats tick, which redraws just the status chip or card. Stats stop while minimised. The spinner is the only continuous animation and only shows while busy. |
| Thumbnails | Generated by the **panel** at import time, so the RAM is freed on close. `.kumo` gets an optional embedded preview image (written by the bake script); fallback is `kumo.exe --shot`. Video: Media Foundation source reader, frame at 10 % of the duration. Image: WIC scaling. Saved as 640×360 JPEG at quality 82 (≈ 30 KB). |
| Library storage | `%LOCALAPPDATA%\Kumo\library\<id>\` holding `item.json`, `thumb.jpg` and the media file if copied, plus `library.json` as the index. Large media must not live in roaming `%APPDATA%`. Settings stay in `%APPDATA%\Kumo\kumo.ini`. |
| Monitor identity | Monitor device path from `QueryDisplayConfig` / `DisplayConfigGetDeviceInfo` (stable across reboots and port changes). Assignments for disconnected monitors are remembered. A new monitor gets the wallpaper of the primary screen. |
| Property schema | Each wallpaper declares its settings (see §9.3). The customise page is generated from that list; there is no hand-made page per scene. |

### Panel RAM budget (target ≤ 40 MB, hard limit 60 MB)

| Part | Budget |
|---|---|
| Runtime, window, software framebuffer (960×640 @150 % ≈ 1440×960×4 = 5.5 MB) | ≈ 15 MB |
| Fonts (only Segoe UI Variable, Segoe UI, Segoe Fluent Icons / MDL2 Assets; do not load every system font) | ≈ 5 MB |
| Thumbnails: visible ones + LRU of 24, decoded at tile size (≈ 340×190×4 = 260 KB each) | ≈ 6–8 MB |
| Import work (video frame grab, JPEG encode), freed afterwards | peaks of +20 MB |

Measure the private working set in the Task Manager "Memory" column, just as the README does.

---

## 4. Visual language

### 4.1 Colour tokens

Neutrals follow Windows 11 Fluent (solid approximations of the Mica/Layer stack), so the Slint Fluent style already matches. The only brand colour is a muted **sakura rose**: warm, calm, used sparingly (selection, primary button, toggles, slider fill, the status dot is *not* accent).

| Token | Dark | Light | Use |
|---|---|---|---|
| `bg.base` | `#202020` | `#F3F3F3` | window background, sidebar |
| `bg.layer` | `#272727` | `#F9F9F9` | content area (top-left corner radius 8, 1 px `stroke.layer`) |
| `bg.card` | `#2D2D2D` | `#FFFFFF` | settings cards, tiles |
| `bg.card.hover` | `#323232` | `#F9F9F9` | |
| `bg.subtle.hover` | `rgba(255,255,255,.06)` | `rgba(0,0,0,.04)` | nav item hover, subtle buttons |
| `bg.subtle.selected` | `rgba(255,255,255,.08)` | `rgba(0,0,0,.06)` | selected nav item |
| `control.fill` | `#373737` | `#FEFEFE` | buttons, combo boxes, text boxes |
| `control.fill.hover` | `#3D3D3D` | `#F9F9F9` | |
| `control.fill.pressed` | `#323232` | `#F3F3F3` | |
| `control.stroke` | `#444444` | `#E0E0E0` (bottom edge `#CCCCCC`) | |
| `control.strong` | `#4A4A4A` | `#FFFFFF` | selected segment, slider thumb outer |
| `control.track` | `#373737` | `#EDEDED` | SegmentedControl track (light mode needs a grey track so the white selected segment stands out) |
| `stroke.card` | `#1C1C1C` | `#E5E5E5` | card and tile borders |
| `stroke.layer` | `#1C1C1C` | `#E5E5E5` | |
| `divider` | `#353535` | `#E5E5E5` | |
| `text.primary` | `#FFFFFF` | `#1A1A1A` | |
| `text.secondary` | `#C5C5C5` | `#5F5F5F` | descriptions (≥ 5.4:1) |
| `text.tertiary` | `#9A9A9A` | `#878787` | placeholders, disabled hints |
| `text.disabled` | `#717171` | `#A0A0A0` | |
| **`accent`** | **`#F2A2BE`** | **`#AD4069`** | primary button, toggle on, slider fill, selection ring, links |
| `accent.hover` | `#F5B3CA` | `#9B385E` | |
| `accent.pressed` | `#E68FAF` | `#8A3254` | |
| `accent.text` (text on accent) | `#2A1520` | `#FFFFFF` | contrast 9.6:1 dark / 5.7:1 light |
| `accent.link` (accent as text) | `#F2A2BE` | `#AD4069` | 8.3:1 on `#202020` / 5.1:1 on `#F3F3F3` |
| `accent.tint` | `rgba(242,162,190,.12)` | `rgba(173,64,105,.08)` | drop zone, pick-mode banner |
| `success` | `#6CCB5F` | `#0F7B0F` | "running" dot, check marks |
| `caution` | `#FCE100` | `#9D5D00` | |
| `critical` | `#FF99A4` | `#C42B1C` | |
| InfoBar backgrounds | info `#2D2D2D` · success `#393D1B` · caution `#433519` · critical `#442726` | info `#F6F6F6` · success `#DFF6DD` · caution `#FFF4CE` · critical `#FDE7E9` | WinUI values |
| Focus ring | outer 2 px `#FFFFFF`, inner 1 px `#000000` | outer 2 px `#1A1A1A`, inner 1 px `#FFFFFF` | keyboard focus only |

Colour mode follows Windows (`AppsUseLightTheme`) unless overridden in Settings. Set `DWMWA_USE_IMMERSIVE_DARK_MODE` on the window so the system title bar matches. Mica (`DWMWA_SYSTEMBACKDROP_TYPE`) is optional on Windows 11 22H2+ and only if the renderer can clear to transparent; the solid colours are the reference.

### 4.2 Typography

Font: **Segoe UI Variable** (Display optical size for ≥ 20 px, Text for smaller). Fallback: Segoe UI. German strings run ~30 % longer than English: never set fixed widths on text, let labels wrap to two lines in cards, and ellipsize only titles in tiles.

| Style | Size / line height | Weight | Use |
|---|---|---|---|
| Caption | 12 / 16 | Regular | meta lines, descriptions under rows, badges |
| Body | 14 / 20 | Regular | default text, controls |
| Body Strong | 14 / 20 | Semibold | tile titles, card titles, section headers |
| Body Large | 18 / 24 | Regular | wizard lead paragraph |
| Subtitle | 20 / 28 | Semibold | dialog titles, detail page wallpaper name |
| Title | 28 / 36 | Semibold | page titles, wizard titles |
| Title Large | 40 / 52 | Semibold | RAM number on the performance page |

Numbers that update (RAM, fps, %) use tabular figures (`font-feature-settings: "tnum"`) so they do not jitter.

### 4.3 Spacing, sizes, radius, elevation

* Base unit **4 px**. Scale: `4 · 8 · 12 · 16 · 20 · 24 · 32 · 48`.
* Page padding: 24 left/right, 20 top, 24 bottom. Settings content max width 1000 px (left aligned).
* Settings card: min height 64, padding 16 horizontal / 12 vertical, icon (20 px) at x 16, text column at x 52, control right-aligned with 16 px inset. **4 px** gap between cards in a group, **24 px** before a section header, **8 px** between header and first card.
* Control height 32 (buttons, combo, text box, segmented). Icon button 32×32 with a 16 px glyph. Toggle 40×20.
* Corner radius: **4** for controls (buttons, inputs, InfoBar, segments, small thumbnails); **8** for cards, tiles, flyouts, dialogs, the monitor canvas and the content layer's top-left corner. The window corners are drawn by Windows.
* Elevation: cards and tiles have a border only, no shadow. Flyouts, menus and dialogs: shadow `0 8 16` at 26 % black (dark) / 14 % (light). The dialog smoke layer is 30 % black.
* Motion: none required. Optional 83 ms colour fades on hover. No page transitions, no looping animation except the busy spinner.

### 4.4 Icons – Segoe Fluent Icons

Built into Windows 11. On Windows 10 use **Segoe MDL2 Assets**, which has the same code points for all glyphs below except `F432` (use `E863`). Render icons as text with the icon font (16 px in controls, 20 px in settings cards, 48 px in empty states).

| Use | Glyph | Code |
|---|---|---|
| Library / Bibliothek | Library | `E8F1` |
| Displays / Bildschirme | TVMonitor | `E7F4` |
| Playback & performance | SpeedHigh | `EC4A` |
| Settings | Settings | `E713` |
| About | Info | `E946` |
| Kumo 3D scene type | Cloud | `E753` |
| Video type | Video | `E714` |
| Image type | Photo | `E91B` |
| Add | Add | `E710` |
| Search | Search | `E721` |
| More (…) | More | `E712` |
| Customise / Anpassen | Personalize | `E771` |
| Play / Pause | Play `E768` · Pause `E769` | |
| Back | Back | `E72B` |
| Breadcrumb / disclosure | ChevronRight `E76C` · ChevronDown `E70D` | |
| Delete / remove | Delete | `E74D` |
| Show in Explorer | FolderOpen | `E838` |
| Time of day | Clock | `E917` |
| Camera orbit | Rotate | `E7AD` |
| Petals | Leaf | `E8BE` |
| Parallax | Mouse | `E962` |
| Frame rate | SpeedHigh | `EC4A` |
| Render resolution | ViewAll | `E8A9` |
| Covered by windows | FullScreen | `E740` |
| Battery | Battery5 | `E855` |
| Energy saver | BatterySaver | `F432` (Win10: `E863`) |
| App rules | Apps | `ED35` |
| Memory | RAM | `EEA0` |
| Screen off / hidden | Hide | `ED1A` |
| Locked | Lock | `E72E` |
| Language | Globe | `E774` |
| Colour mode | Color | `E790` |
| Start with Windows | PowerButton | `E7E8` |
| Log / diagnostics | Diagnostic | `E9D9` |
| Folder | Folder | `E8B7` |
| Updates | Refresh | `E72C` |
| Download | Download | `E896` |
| Source code | Code | `E943` |
| Report a problem | Bug | `EBE8` |
| Uninstall | Uninstall | `EC91` |
| Admin / UAC | Shield | `EA18` (on real buttons prefer the system stock icon `SIID_SHIELD`) |
| Status | CheckMark `E73E` · Warning `E7BA` · Error `E783` · Info `E946` · Cancel `E711` | |
| Volume | Volume `E767` · Mute `E74F` | |
| Tip | Lightbulb | `EA80` |
| Open in new window | OpenInNewWindow | `E8A7` |

App icon: a five-petal sakura blossom in the accent colour, sitting on a soft cloud silhouette, on a transparent background. Needed at 16, 20, 24, 32, 48 and 256 px. The tray keeps the existing cherry-blossom icon.

### 4.5 Thumbnails

* Aspect ratio **16:9** everywhere (tile, detail preview, "now playing" bar, monitor canvas cropped "cover" to the monitor's real aspect).
* Stored at 640×360 JPEG, quality 82. Decoded and scaled to the **physical** display size at load time, never kept at full size.
* Placeholder while generating, or if generation fails: a vertical gradient in the type's hue with the type glyph in the centre (3D `#3A3550→#6B4A6E`, video `#24384A→#3E6077`, image `#3B4A3A→#62775A`).
* Corner radius: tile 8 (top corners only, joined to the card), detail preview 8, small thumbnails 4.

---

## 5. Components

Standard Slint Fluent widgets cover Button, CheckBox, ComboBox, LineEdit, Slider, Switch, ScrollView, ListView, ProgressIndicator and Spinner. The custom components below are thin wrappers built from the same tokens.

| Component | Spec |
|---|---|
| **NavItem** | 36 px high, radius 4, 4 px margin horizontal inside the 208 px sidebar. Icon 16 at x 16, label at x 48 (Body). Hover `bg.subtle.hover`. Selected `bg.subtle.selected` plus a **3×16 px accent pill** at x 0, vertically centred. Compact mode: 40×36, icon only, tooltip with the label. |
| **StatusChip** (sidebar footer) | 32 px, radius 4, `bg.subtle.hover` on hover. 8 px dot (`success` = playing, `text.tertiary` = paused, `caution` = problem) + Caption text "Läuft · 74 MB". Click → Playback & performance. Tooltip shows GPU/CPU. |
| **PageHeader** | Title (28/36) left. Actions right, vertically centred on the title's line. A drill-in page shows a breadcrumb instead: parent in `text.secondary` (clickable), `E76C` at 12 px, current page in `text.primary`. |
| **SettingsCard** | See 4.3. Parts: icon (optional), title (Body), description (Caption, `text.secondary`, wraps), trailing control. Whole card clickable if it navigates (then a trailing `E76C`). Disabled: all text `text.disabled`. |
| **SettingsExpander** | SettingsCard with a trailing `E70D` chevron that rotates to up. Children are inset rows (48 px min, 52 px left inset) separated by 1 px `divider`, sharing the card background. |
| **SegmentedControl** | 32 px, track `control.track` + `control.stroke`, radius 4, 2 px inner padding. Segments size to their label + 24 px. Selected segment: `control.strong` fill, 1 px stroke, Body Strong text and a **3×16 px accent pill** centred on the bottom edge. Keyboard: arrows move selection, which applies immediately. |
| **ToggleSwitch** | 40×20, radius 10. Off: 1 px `text.secondary` stroke, 12 px knob. On: `accent` fill, 14 px knob in `accent.text`. The state label "Ein"/"Aus" sits **left** of the switch (Windows 11 Settings pattern). |
| **Slider** | 4 px track (`control.stroke`), filled part `accent`, 20 px thumb (`control.strong` ring + 12 px accent dot). Value label right of the slider, min width 48, tabular figures. Steps snap; Page Up/Down = 10 %. |
| **InfoBar** | Radius 4, padding 12/16, severity icon 16 px coloured, title Body Strong + message Body on one line, wrapping. Optional action button and close `E711`. Placement: top of the page content, under the header, full width. |
| **WallpaperTile** | See §6.2. |
| **MonitorCanvas** | See §6.4. |
| **Pill/Badge** | 20 px, radius 10, padding 0/8, Caption Semibold. Variants: accent (`accent` / `accent.text`), neutral (`control.strong` / `text.primary`). Over thumbnails add a 40 % black backdrop behind the neutral variant for legibility. |
| **DropOverlay** | Covers the whole content layer while files are dragged over the window: `accent.tint` fill, 2 px dashed `accent` border inset 12 px, radius 8, centre icon `E896` 48 px, title Subtitle, list of formats Caption. Invalid drop: `critical` border, icon `E783`. |
| **MenuFlyout** | `bg.card` background, radius 8, shadow, 4 px padding, items 32 px with a 16 px icon, separators 1 px `divider`. Opens below the anchor or above it if there is no space. |
| **ContentDialog** | Width 400–548, radius 8. Title Subtitle; body Body; footer 80 px on `bg.base` with buttons right-aligned and equal width (primary = accent). Esc = cancel. Smoke layer behind. |
| **ProgressBar** | 3 px (wizard 4 px), radius 2, `accent` on `control.stroke`. Indeterminate only where the duration really is unknown (waiting for UAC). |

---

## 6. Control panel

### 6.1 Window and shell

* **Size:** default 960×640 (outer, effective px; client ≈ 960×608 under the 32 px system title bar). Minimum 720×520. Opens centred on the monitor that holds the tray / mouse cursor; remembers its size and position later (S9).
* **Title bar:** standard Windows title bar, caption "Kumo", app icon. No custom chrome (keeps the toolkit simple and DPI-correct).
* **Layout:**

```
┌────────────────────────────────────────────────────────────────────────┐
│ ❀ Kumo                                               ─   ▢   ✕        │ 32 system title bar
├───────────────┬────────────────────────────────────────────────────────┤
│ ☁ Kumo        │ ╭──────────────────────────────────────────────────────│ bg.layer, r=8 top-left
│               │ │ Page title                              [actions]    │
│ ▌📚 Bibliothek │ │                                                      │
│   🖵 Bildschirme│ │ page content (scrolls)                               │
│   ⚡ Wiedergabe │ │                                                      │
│     & Leistung│ │                                                      │
│               │ │                                                      │
│               │ │                                                      │
│ ─────────────  │ │                                                      │
│ ● Läuft · 74 MB│ │                                                      │
│ ⚙ Einstellungen│ │                                                      │
│ ⓘ Über Kumo   │ │                                                      │
└───────────────┴────────────────────────────────────────────────────────┘
   208 px              content = window − 208
```

* **Sidebar** (208 px, `bg.base`): app header 48 px (blossom icon 20 px + "Kumo" Body Strong at x 16), then nav items **Bibliothek, Bildschirme, Wiedergabe & Leistung**. A flexible gap. Footer: StatusChip, 1 px divider, **Einstellungen**, **Über Kumo**, 12 px bottom padding.
* **Below 860 px** window width the sidebar collapses to compact mode (48 px, icons only, tooltips; the StatusChip becomes just the dot with the number in a tooltip).
* **Content layer** (`bg.layer`) starts at x 208, y 0 with an 8 px top-left radius and a 1 px `stroke.layer` on top and left. Each page scrolls inside the layer; the header scrolls with it (simple implementation).
* **Navigation memory:** within one panel session each page keeps its scroll position. The panel always opens on **Bibliothek**, except when opened through a deep link (`--panel=displays`, `--panel=performance`), e.g. from the tray's status tooltip.

### 6.2 Bibliothek / Library

#### Layout (960×640)

```
Bibliothek                                           [ ＋ Hinzufügen ]
[🔍 Suchen …            ]        [ Alle | 3D | Videos | Bilder ]   (S1)
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ ●Aktiv · 1 2 │ │              │ │              │   tile: 16:9 thumb
│   (thumb)    │ │   (thumb)    │ │   (thumb)    │
├──────────────┤ ├──────────────┤ ├──────────────┤
│Voxel Pagoda… │ │Wolkenmeer    │ │Regen am Fen… │   Body Strong
│☁ 3D-Szene · E…⋯│ │▶ Video · 1080p⋯│ │▶ Video · 4K  ⋯│   Caption + more button
└──────────────┘ └──────────────┘ └──────────────┘
┌──────────────┐ ┌──────────────┐ ┌ ─ ─ ─ ─ ─ ─ ┐
│   (thumb)    │ │   (thumb)    │   ＋ Hinzufügen   dashed "add" tile
…                                  └ ─ ─ ─ ─ ─ ─ ┘
──────────────────────────────────────────────────────────────────────
[thumb] Voxel Pagoda Garden                         [✎ Anpassen] [⏸]   now-playing bar, 56 px
        Läuft auf Bildschirm 1 und 2 · 30 fps
```

* **Header:** Title "Bibliothek". Right: accent button **＋ Hinzufügen** (opens the file picker, multi-select, filter "Alle unterstützten Dateien").
* **Toolbar** (S1, 32 px, 16 px under the header): search box 240 px (`Ctrl+F`) on the left; SegmentedControl type filter on the right. In v0.2 the row can be left out; keep the 16 px rhythm.
* **Grid:** columns = `floor((W + 12) / (200 + 12))`, where W is the content width without padding; tiles stretch to fill. Column gap and row gap are both 12. At 960 px: 3 columns, tiles ≈ 227 px wide. At 1280 px: 4 columns. Order: built-in first, then by date added, newest first. Virtualise by rows.
* **Now-playing bar:** 56 px, pinned to the bottom of the library page (not the window), `bg.layer` with a 1 px top `divider`. Thumb 64×36 radius 4, name Body Strong, status Caption. Buttons: **Anpassen** (standard, icon `E771`) opens the detail page of the active wallpaper; icon button **Pause/Play** (tooltip "Pausieren" / "Fortsetzen"). If the screens show different wallpapers: the name becomes "2 Hintergründe aktiv" and the button "Bildschirme" opens the Displays page.

#### WallpaperTile

| Part | Spec |
|---|---|
| Card | `bg.card`, 1 px `stroke.card`, radius 8. Hover `bg.card.hover`. |
| Thumbnail | 16:9, top corners radius 8. |
| Active pill | Top-left of the thumb, 8 px inset: accent pill **"Aktiv"**. If the screens show different wallpapers, add the screen numbers: "Aktiv · 1, 2". |
| Active ring | 2 px `accent` outline outside the card border (does not change the layout). |
| Body | Padding 10/12. Line 1: title, Body Strong, one line, ellipsis (full title in the tooltip). Line 2: Caption `text.secondary`: type glyph 12 px + type + one fact (3D: "Eingebaut" or file size; video: "1080p · 0:24"; image: "3840 × 2160"). |
| More button | 28×28 subtle icon button `E712` at the right of the body, vertically centred. Always visible (no hover-only controls, so it works for touch and keyboard). |
| Click | **Applies** the wallpaper (see "Apply" below). Double-click: also applies, then opens the detail page. |
| Right-click / Menu key / more button | MenuFlyout: **Anwenden auf ▸** (Alle Bildschirme / Bildschirm 1 / Bildschirm 2 …), **Anpassen …**, **Im Explorer zeigen**, separator, **Aus Bibliothek entfernen** (disabled for built-in, tooltip "Eingebaute Szenen können nicht entfernt werden"). |
| Keyboard | Arrow keys move focus in the grid, Enter applies, `Shift+F10` / Menu key opens the menu, `Del` removes (with dialog), `Ctrl+E` customise. |

#### Apply behaviour

* **Mode "Überall gleich"** (default) or only one monitor: one click sets the wallpaper on every screen that has the live wallpaper switched on. No dialog.
* **Mode "Pro Bildschirm"** with ≥ 2 monitors: the click opens a small MenuFlyout anchored to the tile, titled "Anwenden auf". Items: one per screen ("Bildschirm 1 · 2560 × 1440", with a mini monitor glyph) and "Alle Bildschirme". Picking an item applies it.
* While applying (engine loading a scene or video, normally < 1 s): the tile shows a Spinner in the active pill position. Errors arrive as an InfoBar.
* After applying, **nothing else changes**: no navigation, no toast. The active pill moves and the now-playing bar updates.

#### States

| State | What it looks like |
|---|---|
| **Built-in only** (first start) | The pagoda tile + the dashed **add tile** ("＋ Hinzufügen" / "Video, Bild oder .kumo-Datei hierher ziehen"). Above the grid a dismissible InfoBar (info): "Willkommen bei Kumo – Ziehe ein Video oder Bild in dieses Fenster, um es als Hintergrund zu verwenden." Once dismissed it never returns. |
| **Truly empty** (built-in hidden by a broken install) | Centred empty state: `E753` 48 px in `text.tertiary`, Subtitle "Noch keine Hintergründe", Body "Ziehe ein Video, Bild oder eine .kumo-Datei hierher.", accent button "Datei auswählen". |
| **No search results** | Centred: `E721` 48 px, "Keine Treffer für „regen“", link button "Suche löschen". |
| **Dragging files over the window** | DropOverlay over the content layer: "Zum Hinzufügen loslassen" + "MP4 · WebM · JPG · PNG · WebP · BMP · .kumo". If **any** dragged file is unsupported: critical style, "Dieses Format wird nicht unterstützt" + file name; dropping adds only the supported files. |
| **Importing** | A new tile is inserted at the front with a placeholder thumbnail, a 3 px ProgressBar at the bottom of the thumb and the meta line "Wird hinzugefügt …" (copy + thumbnail), later "Wird umgewandelt … 40 %" (GIF, S7). The rest of the UI stays usable. |
| **Import failed** | The tile is removed and an InfoBar (critical) appears: "„clip.mkv“ konnte nicht hinzugefügt werden. Dieses Format wird nicht unterstützt." For a missing codec: "Für dieses Video (HEVC) fehlt die HEVC-Videoerweiterung von Microsoft." + button "Im Store öffnen". |
| **Heavy video warning** | When the video is > 2560 px wide or > 30 fps, an InfoBar (caution) after import: "4K-Video: Kumo braucht dafür mehr Grafikspeicher. Für die meisten Bildschirme reicht 1080p." No blocking. |
| **Source file missing** (files linked, not copied) | Tile thumb dimmed 50 %, a caution pill "Datei fehlt" replaces the meta line; the click opens a dialog: "Datei nicht gefunden" with "Suchen …" / "Entfernen". |
| **Pick mode** (opened from Displays → "Ändern …") | Accent-tint banner at the top: "Wähle einen Hintergrund für Bildschirm 2" + button "Abbrechen". A click assigns the wallpaper to that screen and returns to Displays. The now-playing bar is hidden. |

#### Remove dialog

Title "„Regen am Fenster“ entfernen?". Body: "Die Kopie in der Kumo-Bibliothek wird gelöscht. Deine Originaldatei bleibt erhalten." If the wallpaper is active, add: "Bildschirm 1 zeigt danach „Voxel Pagoda Garden“." Buttons: **Entfernen** (accent) / Abbrechen. Removing deletes `%LOCALAPPDATA%\Kumo\library\<id>` (moved to the Recycle Bin via `IFileOperation`, never a hard delete).

### 6.3 Detail page – customise a wallpaper

Drill-in from a tile ("Anpassen …"), from the now-playing bar or by double-click. Esc / `Alt+←` / Back button returns to the library.

```
[←] Bibliothek › Voxel Pagoda Garden                         breadcrumb, Title 28
┌──────────────────┐  Voxel Pagoda Garden                     Subtitle 20
│                  │  Typ       3D-Szene (.kumo)              Caption label / value
│ preview 256×144  │  Quelle    Eingebaut · 2,1 MB
│  [Aktiv]         │  Speicher  ca. 60–75 MB gesamt
│                  │  Status    ● Läuft auf Bildschirm 1 und 2
└──────────────────┘  [ Anwenden | ▾ ]  [📂 Im Explorer zeigen]
Anpassen  Änderungen werden sofort auf dem Desktop übernommen.
┌──────────────────────────────────────────────────────────────────────┐
│🕑 Tageszeit                               [Echtzeit | Fest | Zeitraffer]│
│      Uhrzeit                                    ──────●─────  16:30  │  inset row (only "Fest")
│      Morgen 7:30 · Mittag 12:30 · Abendrot 18:36 · Nacht 22:00       │
├──────────────────────────────────────────────────────────────────────┤
│⟳ Kamera dreht sich        0 = steht still        ──●────────  1×     │
├──────────────────────────────────────────────────────────────────────┤
│🍃 Kirschblüten                          [Keine | Wenige | Normal | Viele]│
├──────────────────────────────────────────────────────────────────────┤
│🖱 Parallaxe   Die Kamera folgt leicht der Maus.             Aus [  ○] │
└──────────────────────────────────────────────────────────────────────┘
Auf Standard zurücksetzen
```

* **Hero row** (top, 20 px gap between columns): preview 256×144 (radius 8, 1 px `stroke.card`, active pill). To its right: name (Subtitle), then the info list (two columns: Caption label `text.secondary`, Caption value, gap 4/16): Typ, Quelle (Eingebaut / file name) · Größe, Auflösung/Dauer (video), **Speicher** (estimate, see below), Status. Then the button row: accent **split button "Anwenden"** (main part = same behaviour as a tile click; the arrow lists the screens) and a standard button `E838` "Im Explorer zeigen". Built-in items get no remove button; others get a subtle "Aus Bibliothek entfernen" at the end of the row.
* **Customise section** (full content width, so segmented controls and sliders fit next to their labels at 960 px): header "Anpassen" (Body Strong) with the Caption "Änderungen werden sofort auf dem Desktop übernommen." on the same line, then one SettingsCard per property (§9.3), then a hyperlink button "Auf Standard zurücksetzen" (resets only this wallpaper's properties, no dialog, undo is trivial). Sliders are 180 px wide in every row so the value labels line up.
* Under 860 px window width the hero row stacks (preview on top, max width 480); cards whose control does not fit move the control under the title (card min height grows).
* If the wallpaper is **not active**, the property cards still work (values are saved) and a Caption at the top says: "Dieser Hintergrund ist gerade nicht aktiv. Deine Einstellungen werden beim Anwenden verwendet."
* **Memory estimate row**: 3D scene: measured value stored in the scene manifest ("ca. 60–75 MB gesamt"). Video: `width × height × 1.5 bytes × 8 surfaces` plus a fixed 20 MB, rounded ("ca. +35 MB"). Image: "0 MB – wird von Windows angezeigt". Tooltip: "Schätzung. Die echte Zahl siehst du unter Wiedergabe & Leistung."

#### Properties per type

| Type | Cards |
|---|---|
| **Voxel Pagoda Garden** (`.kumo`, from its manifest) | **Tageszeit** `E917` – Segmented *Echtzeit · Fest · Zeitraffer*. With *Fest*: an inset row "Uhrzeit" with a slider 0–24 h, step 15 min, value "16:30", plus presets as link buttons *Morgen 7:30 · Mittag 12:30 · Abendrot 18:36 · Nacht 22:00* (the same values as the tray). With *Zeitraffer*: inset row "Geschwindigkeit" ComboBox *1 Tag in 24 min (60×) · 1 Tag in 6 min (240×) · 1 Tag in 2,4 min (600×) · 1 Tag in 36 s (2400×)*. ‖ **Kamera dreht sich** `E7AD` – slider 0–3×, step 0.25, "0 = steht still". ‖ **Kirschblüten** `E8BE` – Segmented *Keine · Wenige · Normal · Viele* (`petals` 0 / 0.5 / 1.0 / 1.75). ‖ **Parallaxe** `E962` – toggle, description "Die Kamera folgt leicht der Maus." |
| **Video** | **Anpassung** – ComboBox *Füllen (zuschneiden) · Einpassen (Ränder) · Strecken*. ‖ **Ton** (S2) – toggle "Ton abspielen" + inset volume slider; description "Nur hörbar, solange der Desktop sichtbar ist." ‖ Status line (Caption, with icon): `E73E` success "Hardware-beschleunigt (H.264)" **or** `E7BA` caution "Ohne Hardware-Beschleunigung – höhere CPU-Last". |
| **Image** | **Anpassung** – ComboBox *Füllen · Einpassen · Strecken · Zentriert · Kacheln* (the Windows `DESKTOP_WALLPAPER_POSITION` values). ‖ InfoBar (success, not closable): "Bilder zeigt Windows selbst an – das kostet keinen zusätzlichen Speicher." |
| **Web** (Later) | InfoBar (caution) at the top: "Kompatibilitätsmodus: Web-Hintergründe nutzen einen eingebauten Browser und brauchen ca. 150–400 MB Arbeitsspeicher." |

### 6.4 Bildschirme / Displays

```
Bildschirme                                            [ Identifizieren ]
┌───────────────────────────────────────────────────────────────────────┐
│ 🖵 Hintergrund auf mehreren Bildschirmen  [Überall gleich|Pro Bildschirm]│
└───────────────────────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────────────────────┐
│             ┏━━━━━━━━━━━━━━━━━┓┌────────────┐                         │  MonitorCanvas
│             ┃  (thumb cover)  ┃│ (thumb)    │                         │  200 px high
│             ┃ ①               ┃│ ②          │                         │
│             ┗━━━━━━━━━━━━━━━━━┛└────────────┘                         │
└───────────────────────────────────────────────────────────────────────┘
Bildschirm 1 · Hauptbildschirm
DELL S2721DGF · 2560 × 1440 · Skalierung 125 %
┌───────────────────────────────────────────────────────────────────────┐
│ [thumb] Hintergrund     Voxel Pagoda Garden              [ Ändern … ] │
├───────────────────────────────────────────────────────────────────────┤
│ 👁 Live-Hintergrund auf diesem Bildschirm                    Ein [●]  │
├───────────────────────────────────────────────────────────────────────┤
│ ⟲ Abwechseln (S3)             [ Nie ▾ ]                               │
└───────────────────────────────────────────────────────────────────────┘
ⓘ Zwei verschiedene Hintergründe: Kumo braucht gerade 96 MB statt 74 MB.
```

* **Header action:** "Identifizieren": the engine shows a large number (Title Large in a 120 px rounded accent square, 3 s) in the centre of every monitor. In the panel the same numbers are on the canvas.
* **Mode card:** SettingsCard "Hintergrund auf mehreren Bildschirmen" with the Segmented *Überall gleich · Pro Bildschirm* (Later: *Strecken*). Hidden when only one monitor exists; instead a Caption: "Schließe einen zweiten Bildschirm an, um ihm einen eigenen Hintergrund zu geben."
* **MonitorCanvas:** a card, height 200, padding 20. Monitors are drawn as rectangles in their real virtual-desktop arrangement (`EnumDisplayMonitors` rects in physical pixels), uniformly scaled to fit, centred, 4 px gap between touching monitors. Each monitor: radius 4, thumbnail cropped to its aspect, 1 px `control.stroke`; number badge (24 px circle, `control.strong`, Body Strong) at the bottom-left, 8 px inset. **Selected**: 2 px `accent` border + the badge in accent. **Live wallpaper off**: `bg.base` fill, `ED1A` 20 px centred, Caption "Aus". **Paused because covered**: a small neutral pill "Pausiert" top-right. Click/arrow keys select a monitor (always possible, also in "Überall gleich" mode, so its per-screen toggle can be reached).
* **Screen detail** (under the canvas): heading "Bildschirm 1" (Body Strong) + "· Hauptbildschirm" for the primary one; Caption: monitor name · resolution · scaling. Cards:
  * **Hintergrund**: thumb 80×45 radius 4 + name, button "Ändern …" → library in pick mode. In "Überall gleich" mode the card reads "Alle Bildschirme" and changes all of them.
  * **Live-Hintergrund auf diesem Bildschirm**: toggle. Description when off: "Windows zeigt hier dein normales Hintergrundbild. Spart Speicher und Strom." At least one screen must stay on; otherwise the toggle reads "Alle aus – nutze ‚Pausieren‘ oder beende Kumo" (disabled).
  * **Anpassung** (only when that screen shows a video or image): fit ComboBox.
  * **Abwechseln** (S3): ComboBox *Nie · Alle 15 min · Alle 30 min · Jede Stunde · Nach Tageszeit …*.
* **Memory InfoBar** (info) whenever ≥ 2 distinct non-image wallpapers are active: "Unterschiedliche Hintergründe brauchen mehr Speicher – gerade 96 MB statt 74 MB. Gleiche Hintergründe auf Bildschirmen mit gleichem Seitenverhältnis teilen sich ein Bild." (live numbers from `stats`; the "statt" value is the last measured figure with one shared wallpaper, otherwise the sentence ends after "Speicher").
* **Hot-plug InfoBar** (info, closable) when a new monitor appears while the panel is open: "Neuer Bildschirm erkannt. Er zeigt denselben Hintergrund wie Bildschirm 1." + "Ändern".

### 6.5 Wiedergabe & Leistung / Playback & performance

```
Wiedergabe & Leistung
┌───────────────────────────────────────────────────────────────────────┐
│  74 MB              GPU  6 %     CPU  0,4 %        ● Läuft            │
│  Arbeitsspeicher    2 Bildschirme · 30 fps                [⏸ Pausieren]│
│  Dieses Fenster braucht zusätzlich ca. 30 MB und wird beim Schließen   │
│  komplett beendet.                                                    │
└───────────────────────────────────────────────────────────────────────┘
Bildqualität
│⚡ Bildrate                                           [15 | 30 | 60] fps│
│▦ Renderauflösung              [Hoch 100 % | Ausgewogen 80 % | Sparsam 62 %]│
│  Erweitert ▾ : Kantenglättung (FXAA)                           Ein [●]│
Automatisch pausieren
│⛶ Wenn Fenster den Bildschirm verdecken   [Bei Vollbild und maximiert ▾]│
│🔋 Im Akkubetrieb                            [Bildrate halbieren ▾]     │
│🍃 Mit Energiesparmodus                       [Pausieren ▾]              │
│▭ Nur auf dem freien Desktop abspielen (S5)                   Aus [○]  │
│▦ App-Regeln (S4) ▾                                                     │
🔒 Immer pausiert: gesperrter PC, ausgeschalteter Bildschirm, Remotedesktop.
```

* **Resource card** (card, padding 20): RAM in Title Large (tabular), label "Arbeitsspeicher" Caption. GPU and CPU in Body Strong with Caption labels. The status pill on the right (success dot "Läuft", neutral "Pausiert", caution "Teilweise pausiert – Bildschirm 2 verdeckt"), and a status line "2 Bildschirme · 30 fps" (actual average fps). Button **Pausieren / Fortsetzen** (standard, with icon). Footer Caption: "Dieses Fenster braucht zusätzlich ca. 30 MB und wird beim Schließen komplett beendet." (the panel's own working set, measured). Values update every 2 s; on a stats timeout show "–".
* **Bildqualität:**
  * **Bildrate** – Segmented *15 · 30 · 60* + trailing Caption "fps". Description: "30 fps wirkt bei Hintergründen flüssig und braucht etwa halb so viel Grafikleistung wie 60."
  * **Renderauflösung** – Segmented *Hoch (100 %) · Ausgewogen (80 %) · Sparsam (62 %)* (stacks under the title below 860 px). Description: "Niedriger spart Grafikleistung, das Bild wird etwas weicher."
  * **Erweitert** (expander, collapsed): *Kantenglättung (FXAA)* toggle, description "Glättet Treppenstufen an Kanten. Kostet sehr wenig."
* **Automatisch pausieren:**
  * **Wenn Fenster den Bildschirm verdecken** – ComboBox *Bei Vollbild und maximierten Fenstern (empfohlen) · Nur bei Vollbild · Nie*. Description: "Ein verdeckter Bildschirm wird nicht gezeichnet."
  * **Im Akkubetrieb** – ComboBox *Normal weiterlaufen · Bildrate halbieren · Pausieren · Standbild (Speicher freigeben)* (S6 option shown only when implemented). Default **Bildrate halbieren**. Hidden on PCs without a battery (`GetSystemPowerStatus` → `BatteryFlag == 128`).
  * **Mit Energiesparmodus** – same options. Default **Bildrate halbieren** (today's behaviour). Proposal for the owner: default **Pausieren**, which is better for the "saves battery" promise.
  * **Nur auf dem freien Desktop abspielen** (S5) – toggle, description "Pausiert, sobald ein anderes Fenster aktiv ist."
  * **Pausieren gilt für** (S10, inside an expander "Weitere Regeln") – Segmented *Nur den verdeckten Bildschirm · Alle Bildschirme*.
  * **App-Regeln** (S4) – expander. Rows: app icon 16 px + name + exe path Caption + ComboBox *Pausieren · Standbild* + delete icon button. Footer row: button "App hinzufügen …" (file picker for `.exe`, or a list of running apps).
  * Footer Caption with `E72E`: "Immer pausiert: gesperrter PC, ausgeschalteter Bildschirm und Remotedesktop-Sitzungen." No setting.
* **What "pause" means** (tooltip on the info icon next to the section header): "Pausiert: Das letzte Bild bleibt stehen, Kumo braucht dann 0 % CPU und GPU. Standbild: Kumo gibt zusätzlich seinen Grafikspeicher frei und zeigt ein Foto der Szene als Windows-Hintergrund."

### 6.6 Einstellungen / Settings

| Section | Card | Control | Default |
|---|---|---|---|
| **Allgemein** | Mit Windows starten (`E7E8`) – "Kumo startet nach der Anmeldung unauffällig im Hintergrund." | Toggle | On (from the wizard) |
| | Sprache (`E774`) – "Gilt für dieses Fenster und das Menü im Infobereich." | ComboBox *Wie Windows · Deutsch · English* | Wie Windows |
| | Farbmodus (`E790`) | ComboBox *Wie Windows · Hell · Dunkel* | Wie Windows |
| **Bibliothek** | Dateien beim Hinzufügen kopieren (`E8B7`) – "Der Hintergrund funktioniert dann auch, wenn du die Originaldatei verschiebst oder löschst." | Toggle | On |
| | Speicherort – path as Caption, "Belegt: 1,2 GB" | Button "Ordner öffnen" | – |
| **Beim Beenden** (S6) | Windows-Hintergrund nach dem Beenden | ComboBox *Vorheriges Bild wiederherstellen · Standbild des Live-Hintergrunds* | Vorheriges Bild |
| **Problembehandlung** | Protokolldatei (`E9D9`) – "Hilfreich für Fehlerberichte." | Button "Öffnen" | – |
| | Einstellungsordner | Button "Öffnen" | – |
| | Kumo neu starten – "Startet den Hintergrund neu, ohne dass du dich abmelden musst." | Button "Neu starten" | – |
| | Alle Einstellungen zurücksetzen | Button "Zurücksetzen …" → dialog | – |

Language change: the panel re-renders its strings immediately (no restart); the engine updates the tray menu on the next open.

**Reset dialog:** Title "Alle Einstellungen zurücksetzen?" Body "Bildrate, Qualität, Pausenregeln und die Zuordnung der Bildschirme werden auf den Standard gesetzt. Deine Bibliothek bleibt erhalten." Buttons **Zurücksetzen** / Abbrechen.

### 6.7 Über Kumo / About

* Hero card: app icon 64 px, "Kumo" (Title), "Version 0.2.0" (Caption), Body: "Leichtgewichtige Live-Hintergründe für Windows. 雲 (kumo) ist Japanisch für „Wolke“."
* **Updates** card (S8): button "Nach Updates suchen" → Spinner + "Suche nach Updates …" → `E73E` "Kumo ist auf dem neuesten Stand." / accent button "Version 0.3.0 herunterladen" (opens the release page in the browser) / `E7BA` "Keine Verbindung zu GitHub." Caption: "Kumo prüft nur, wenn du hier klickst."
* Link cards (each opens the browser, trailing `E8A7`): Quellcode auf GitHub (`E943`), Problem melden (`EBE8`) – "Hänge am besten die Protokolldatei an.", Lizenz, Hinweise zu Drittanbieter-Software.
* Footer Caption: "Kein Konto, keine Werbung, keine Telemetrie."
* Subtle button at the bottom: "Kumo deinstallieren …" (`EC91`) → opens `ms-settings:appsfeatures` (Windows does the uninstall and the UAC prompt).

### 6.8 Global states

| Situation | Presentation |
|---|---|
| Engine not running (crashed, quit, panel started on its own) | InfoBar (critical) on every page: "Kumo läuft gerade nicht – dein Live-Hintergrund ist aus." + button "Kumo starten". StatusChip: grey dot "Beendet". The panel is a Program Files process, so it may start the engine. |
| Engine cannot attach to the desktop (access denied) | InfoBar (critical): "Kumo konnte sich nicht hinter die Desktop-Symbole legen." + "Details" (opens the log) + "Erneut versuchen". |
| Another wallpaper app running (M13) | InfoBar (caution): "Lively Wallpaper läuft ebenfalls. Zwei Hintergrund-Programme stören sich gegenseitig – beende Lively über sein Symbol im Infobereich." + "Erneut prüfen". Kumo never kills other apps. |
| First start after setup | Library InfoBar "Willkommen …" (see 6.2) + a one-time teaching tip on the StatusChip: "Kumo läuft im Hintergrund weiter, auch wenn du dieses Fenster schließt. Du findest es unten rechts im Infobereich (Kirschblüte)." |
| Paused globally | StatusChip "Pausiert · 41 MB", now-playing bar caption "Pausiert", play button. |

### 6.9 Keyboard, accessibility, DPI

* Tab order: sidebar → page header actions → content, top to bottom. `Ctrl+1/2/3` jump to the three main pages, `Ctrl+,` Settings, `Alt+←` back, `Ctrl+F` search, `Ctrl+O` add, `Space` pause/resume when focus is not in a text box.
* Every icon-only button has an accessible name and a tooltip (same text). The tiles expose "Voxel Pagoda Garden, 3D-Szene, aktiv auf Bildschirm 1 und 2".
* Focus ring only for keyboard focus (§4.1). All text contrast ≥ 4.5:1 (checked for the tokens in §4.1).
* Per-monitor DPI v2; all sizes are effective pixels. Test at 100, 125, 150 and 200 %, and with Windows text scaling at 150 % (cards grow in height, nothing is clipped).
* High contrast (S12): use system colours (`COLOR_WINDOW`, `COLOR_HIGHLIGHT` …) and draw thumbnails with a 1 px `COLOR_WINDOWTEXT` border.

---

## 7. Tray menu (engine, revised)

Left-click on the icon opens the panel. Right-click shows:

| DE | EN |
|---|---|
| **Kumo öffnen** (default item, bold) | **Open Kumo** |
| Pausieren / Fortsetzen | Pause / Resume |
| Bildrate ▸ 15 fps · 30 fps · 60 fps | Frame rate ▸ … |
| ─ | ─ |
| Beenden | Quit |

Tooltip: "Kumo – Voxel Pagoda Garden" / "Kumo – pausiert". Time of day, quality, petals and camera move into the panel; the tray stays short and fast. (Optional for power users: keep "Tageszeit ▸", since it costs nothing.)

---

## 8. Setup wizard

> **Changed in 0.2.2:** Kumo installs per user into `%LOCALAPPDATA%\Programs\Kumo`, needs no
> administrator rights and starts itself after the install. The UAC steps, the Start menu page W4
> and the start through Explorer in this section no longer apply. The earlier belief that live
> wallpapers need Program Files came from testing in a folder with a Low integrity label.

### 8.1 Window

* **Client 640×440**, fixed (no resize/maximise), centred on the primary monitor, standard title bar "Kumo installieren" / "Install Kumo".
* Layout: **art panel** on the left, 200×440: the pagoda artwork (a 200×440 crop of `docs/preview.png`, about 30 KB JPEG), wordmark "Kumo" (Subtitle, white) and "Live-Hintergründe" (Caption, 80 % white) at the bottom-left with 24 px inset; a dark 0→45 % gradient behind the text. **Content** on the right, 440 wide: padding 32 left/right, 32 top. Step caption (Caption, `text.secondary`, e.g. "Schritt 2 von 3"), page title (Title 28), body. **Footer** 64 px across the content column, `bg.base`, 1 px top divider: on the left a small language ComboBox (*Deutsch · English*, borderless, 120 px), buttons on the right (min width 96, gap 8, primary = accent, rightmost).
* Enter = primary, Esc = Cancel (asks "Installation abbrechen?" only while it is running).
* It is the same `kumo.exe` the user downloaded: it shows the wizard when started outside Program Files.

### 8.2 Install flow

```
W1 Willkommen ─▶ W2 Optionen ─▶ [UAC] ─▶ W3 Installieren ─▶ W4 Fertig (A or B) ─▶ W5 Kumo läuft
                      ▲                         │ UAC cancelled
                      └──── error state ◀───────┘
```

| Page | Content | Buttons |
|---|---|---|
| **W1 Willkommen** | Caption "Schritt 1 von 3". Title "Willkommen bei Kumo". Body Large: "Kumo zeigt animierte 3D-Szenen, Videos und Bilder als Desktop-Hintergrund – mit einem Bruchteil des Arbeitsspeichers." Three rows with icons: `EEA0` "Rund 70 MB statt mehrerer hundert MB", `E769` "Pausiert von selbst, wenn du arbeitest oder spielst", `EB51` "Kostenlos, Open Source, ohne Konto und Werbung". If Lively/WE is running: InfoBar (caution) "Lively Wallpaper läuft gerade. Beende es vor der Installation, damit sich beide Programme nicht stören." | Abbrechen · **Weiter** |
| **W2 Optionen** | "Schritt 2 von 3". Title "So wird Kumo eingerichtet". CheckBox ☑ **Mit Windows starten** – "Empfohlen. Sonst musst du Kumo nach jedem Neustart selbst öffnen." CheckBox ☐ **Verknüpfung auf dem Desktop**. Info row `E8B7`: "Installationsordner: C:\Program Files\Kumo" – Caption "Fest vorgegeben: Windows erlaubt Live-Hintergründe nur für Programme aus diesem Ordner." Info row `E8F1`: "Startmenü-Eintrag und Eintrag unter Einstellungen › Apps werden immer angelegt. Benötigt ca. 6 MB." Last row of the content, pinned to the bottom just above the footer, `EA18` in accent: "Windows fragt gleich einmal nach Administratorrechten." (It does not fit in the footer next to the buttons in German.) | Zurück · **🛡 Installieren** |
| **W3 Installieren** | "Schritt 3 von 3". Title "Kumo wird installiert …". Before UAC is answered: indeterminate ProgressBar + "Bitte bestätige die Abfrage von Windows." Afterwards: a determinate bar + checklist (pending ○ / done `E73E` success / current Spinner): *Dateien kopieren · Startmenü-Eintrag anlegen · Autostart einrichten · Bei „Apps“ eintragen*. Shown for at least 600 ms so it does not flicker. | Abbrechen (disabled while copying) |
| **W3 error** | Title "Installation nicht abgeschlossen". InfoBar (critical): "Ohne Administratorrechte kann Kumo nicht in „Programme“ installiert werden." Or for a copy error: "Eine Datei konnte nicht geschrieben werden." + "Details" (log). | Abbrechen · **Erneut versuchen** |
| **W4-A Fertig – Start menu** (the safe default) | Title "Fast geschafft!". Body "Starte Kumo jetzt einmal über das Startmenü:". A row of three key chips (28 px, `control.fill`, 2 px bottom border, Body Strong): **[⊞ Windows-Taste] → [Kumo] → [Enter ↵]**; their accessible text is the three steps `w4a.s1–s3`. Under it a static illustration (376×~90, drawn with normal widgets): a Start menu search box with "Kumo" typed (accent bottom border) and one highlighted result row with the blossom icon, "Kumo · App". Expander "Warum kann ich Kumo nicht hier starten?" → "Windows lässt Live-Hintergründe nur von Programmen aus dem Ordner „Programme“ starten – nicht von dieser heruntergeladenen Datei. Ab der nächsten Anmeldung startet Kumo automatisch." At the bottom: Spinner + Caption "Warte auf Kumo …" (the wizard polls every 500 ms for the engine's named mutex / pipe) → switches to W5. | **Schließen** |
| **W4-B Fertig – start through Explorer** (if the experiment in §8.5 works) | Title "Kumo ist installiert". Body "Kumo startet ab jetzt automatisch mit Windows." Large accent button **"Kumo jetzt starten"** (40 px high, full content width). Clicking shows Spinner "Kumo wird gestartet …". Success within 5 s → W5. Failure or timeout → the page turns into W4-A with an InfoBar (info) at the top: "Windows hat den Start von hier blockiert. Kein Problem – starte Kumo über das Startmenü:" | Schließen · (the big button) |
| **W5 Kumo läuft** | `E73E` 48 px success. Title "Kumo läuft!". Body "Dein neuer Hintergrund ist da. Die Kirschblüte unten rechts im Infobereich öffnet Kumo – auch später jederzeit." Caption "Die heruntergeladene Datei brauchst du nicht mehr und kannst sie löschen." | Schließen · **Kumo öffnen** (asks the running engine over the pipe to open the panel) |

### 8.3 Update, repair, downgrade

Detection: read `DisplayVersion` from `HKLM\…\Uninstall\Kumo` and compare it with the wizard's own version.

| Case | W1 variant | Flow |
|---|---|---|
| **Newer version** | Title "Kumo aktualisieren". A version row "0.1.0 → 0.2.0", Body "Deine Einstellungen und deine Bibliothek bleiben erhalten." Link "Was ist neu?" | **Aktualisieren** (shield) → W3 with the checklist *Kumo kurz beenden · Dateien ersetzen · Kumo neu starten* → W5 "Kumo ist auf dem neuesten Stand." |
| **Same version** | Title "Kumo 0.2.0 ist bereits installiert". Two radio cards: (•) **Reparieren** – "Dateien neu kopieren. Einstellungen bleiben." ( ) **Deinstallieren**. | **Weiter** |
| **Older wizard** | InfoBar (caution) "Installiert ist eine neuere Version (0.3.0). Diese Datei ist älter." | Schließen only (no downgrade in v0.2) |

**Restart after an update without the Start menu step:** the *running* engine is already a trusted Program Files process. The wizard (elevated helper) renames `kumo.exe` → `kumo.old.exe` (Windows allows renaming a running exe), copies the new `kumo.exe` and sends `update-restart` over the pipe. The old engine starts the new exe and exits; the new one deletes `kumo.old.exe`. If the engine was not running, show W4-A/B as on a fresh install. While the engine is restarting the user briefly sees the normal Windows wallpaper. The checklist row says "Kumo kurz beenden (dein Hintergrund ist kurz normal)".

### 8.4 Uninstall

Started from *Einstellungen › Apps › Kumo Live Wallpaper › Deinstallieren* (`kumo.exe --uninstall`, Windows elevates it). Window **440×260**, same tokens, no art panel:

* Title "Kumo deinstallieren?" Body "Kumo wird beendet und entfernt. Dein vorheriges Windows-Hintergrundbild kommt sofort zurück."
* CheckBox ☐ "Auch meine Einstellungen und Bibliothek löschen (1,2 GB)". Default **off**; keeping the data makes a later reinstall seamless.
* Buttons: Abbrechen · **Deinstallieren**.
* Done: `E73E` + "Kumo wurde entfernt." + **Schließen**. The engine restores the original Windows wallpaper (saved at first run) before it exits. The program folder is removed after exit (`MoveFileEx … MOVEFILE_DELAY_UNTIL_REBOOT` as the fallback).

### 8.5 Engineering experiment for W4-B

Find out whether Windows 11 still treats the process as "from the download" in these cases (check with the existing desktop attach test):

1. `ShellExecute` through Explorer's own process: `IShellWindows` → desktop `IShellFolderViewDual` → `Application` → `IShellDispatch2::ShellExecute("C:\Program Files\Kumo\kumo.exe")`. The new process's parent is `explorer.exe`, with medium integrity.
2. `explorer.exe "C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Kumo.lnk"`.
3. A one-shot scheduled task created during install (`schtasks /run`), with `svchost` as the parent.

If any of them works reliably, ship W4-B with the automatic fallback to W4-A. Otherwise ship W4-A only. The UI for both exists.

### 8.6 Before the wizard: SmartScreen

Until the exe is signed, Windows shows "Der Computer wurde durch Windows geschützt". The wizard cannot help with that; the download page/README needs a two-step picture: "Weitere Informationen" → "Trotzdem ausführen".

---

## 9. State and data the UI reads/writes

### 9.1 New/changed keys in `kumo.ini` (owned by the engine; the panel sends `set` over the pipe)

```ini
fps = 30                  # 15 | 30 | 60
quality = 1.0             # 1.0 | 0.8 | 0.62
fxaa = on
pause_covered = maximized # maximized | fullscreen | never
pause_battery = half      # normal | half | pause | still
pause_saver = half        # normal | half | pause | still
pause_scope = screen      # screen | all            (S10)
pause_unfocused = off     #                         (S5)
layout = same             # same | per-screen        (span later)
language = system         # system | de | en
theme = system            # system | light | dark
copy_imports = on
on_quit = restore         # restore | still          (S6)
panel_rect = 120,80,960,640                          (S9)
welcome_done = off
```

### 9.2 `state.json` (screen assignment, `%APPDATA%\Kumo`)

```json
{ "layout": "same",
  "all": "builtin:pagoda",
  "screens": {
    "\\\\?\\DISPLAY#DEL40F6#…": { "wallpaper": "builtin:pagoda", "enabled": true, "fit": "fill" },
    "\\\\?\\DISPLAY#GSM5B7F#…": { "wallpaper": "lib:7f3a…",      "enabled": true, "fit": "fill" } },
  "original_wallpapers": { "\\\\?\\DISPLAY#DEL40F6#…": "C:\\Users\\…\\img0.jpg" } }
```

### 9.3 Wallpaper property schema (in the `.kumo` manifest, and synthesised for video/image)

```json
"properties": [
  { "id": "time", "type": "choice", "icon": "E917",
    "label": { "de": "Tageszeit", "en": "Time of day" },
    "options": [ { "value": "real",  "de": "Echtzeit",  "en": "Real time" },
                 { "value": "fixed", "de": "Fest",      "en": "Fixed" },
                 { "value": "lapse", "de": "Zeitraffer","en": "Time-lapse" } ],
    "default": "real" },
  { "id": "hour", "type": "slider", "min": 0, "max": 24, "step": 0.25, "format": "time",
    "visible_if": { "time": "fixed" }, "parent": "time",
    "label": { "de": "Uhrzeit", "en": "Time" }, "default": 16.5,
    "presets": [ [7.5,"Morgen","Morning"], [12.5,"Mittag","Noon"], [18.6,"Abendrot","Sunset"], [22,"Nacht","Night"] ] },
  { "id": "lapse", "type": "choice", "control": "combo", "parent": "time", "visible_if": { "time": "lapse" },
    "label": { "de": "Geschwindigkeit", "en": "Speed" }, "default": 600,
    "options": [ {"value":60,"de":"1 Tag in 24 min","en":"1 day in 24 min"}, {"value":240,"de":"1 Tag in 6 min","en":"1 day in 6 min"},
                 {"value":600,"de":"1 Tag in 2,4 min","en":"1 day in 2.4 min"}, {"value":2400,"de":"1 Tag in 36 s","en":"1 day in 36 s"} ] },
  { "id": "rotate", "type": "slider", "min": 0, "max": 3, "step": 0.25, "format": "factor", "icon": "E7AD",
    "label": { "de": "Kamera dreht sich", "en": "Camera orbits" },
    "description": { "de": "0 = steht still", "en": "0 = still" }, "default": 1 },
  { "id": "petals", "type": "choice", "icon": "E8BE", "label": { "de": "Kirschblüten", "en": "Cherry blossoms" },
    "options": [ {"value":0,"de":"Keine","en":"None"}, {"value":0.5,"de":"Wenige","en":"Few"},
                 {"value":1,"de":"Normal","en":"Normal"}, {"value":1.75,"de":"Viele","en":"Many"} ], "default": 1 },
  { "id": "parallax", "type": "toggle", "icon": "E962", "label": { "de": "Parallaxe", "en": "Parallax" },
    "description": { "de": "Die Kamera folgt leicht der Maus.", "en": "The camera follows the mouse a little." }, "default": false }
],
"memory_mb": [60, 75],
"preview": "embedded"
```

Rendering rules: `choice` with ≤ 4 short options → SegmentedControl, otherwise ComboBox (or `"control"` forces one). `slider` → Slider with a value label (`format`: `time` → "16:30", `factor` → "1,5×", `percent`). `toggle` → ToggleSwitch. `parent` + `visible_if` → inset row inside the parent's card.

---

## 10. Microcopy

### 10.1 Style guide

* **German:** informal "du" (as in the existing tray texts), short sentences, verbs first on buttons ("Hinzufügen", "Anwenden"). German quotation marks „…“. A space before units and % ("30 %", "74 MB"; a non-breaking space in code: `\u{202F}` or `\u{00A0}`). Decimal comma ("0,4 %", "2,4 min"). 24-hour time ("16:30"). Ellipsis "…" only on commands that open a dialog or picker ("Ändern …" with a space, per Microsoft German style).
* **English:** sentence case everywhere ("Playback & performance"), no "please" except in the UAC wait line, "…" without a space.
* Use **Hintergrund** (wallpaper), **Live-Hintergrund** (live wallpaper), **Bildschirm** (display, as in Windows), **Infobereich** (notification area), **Bildrate** (frame rate). Never "Desktop-Wallpaper", "Monitor" (only in technical captions) or "System Tray".
* Numbers instead of adjectives when talking about cost: "ca. +35 MB", not "etwas mehr Speicher".

### 10.2 String table (key · DE · EN)

#### Shell

| Key | DE | EN |
|---|---|---|
| `app.title` | Kumo | Kumo |
| `nav.library` | Bibliothek | Library |
| `nav.displays` | Bildschirme | Displays |
| `nav.performance` | Wiedergabe & Leistung | Playback & performance |
| `nav.settings` | Einstellungen | Settings |
| `nav.about` | Über Kumo | About Kumo |
| `status.running` | Läuft · {mb} MB | Running · {mb} MB |
| `status.paused` | Pausiert · {mb} MB | Paused · {mb} MB |
| `status.partial` | Teilweise pausiert | Partly paused |
| `status.stopped` | Beendet | Not running |
| `status.tooltip` | GPU {gpu} % · CPU {cpu} % · {fps} fps | GPU {gpu}% · CPU {cpu}% · {fps} fps |
| `common.on` / `common.off` | Ein / Aus | On / Off |
| `common.cancel` | Abbrechen | Cancel |
| `common.close` | Schließen | Close |
| `common.open` | Öffnen | Open |
| `common.back` | Zurück | Back |

#### Library

| Key | DE | EN |
|---|---|---|
| `lib.title` | Bibliothek | Library |
| `lib.add` | Hinzufügen | Add |
| `lib.search` | Suchen … | Search… |
| `lib.filter.all` / `3d` / `video` / `image` | Alle / 3D / Videos / Bilder | All / 3D / Videos / Images |
| `lib.type.scene` | 3D-Szene | 3D scene |
| `lib.type.video` | Video | Video |
| `lib.type.image` | Bild | Image |
| `lib.builtin` | Eingebaut | Built-in |
| `lib.active` | Aktiv | Active |
| `lib.active.screens` | Aktiv · {list} | Active · {list} |
| `lib.addtile.title` | Hinzufügen | Add |
| `lib.addtile.hint` | Video, Bild oder .kumo-Datei hierher ziehen | Drop a video, image or .kumo file here |
| `lib.welcome` | Willkommen bei Kumo – Ziehe ein Video oder Bild in dieses Fenster, um es als Hintergrund zu verwenden. | Welcome to Kumo – drag a video or image into this window to use it as your wallpaper. |
| `lib.empty.title` | Noch keine Hintergründe | No wallpapers yet |
| `lib.empty.body` | Ziehe ein Video, Bild oder eine .kumo-Datei hierher. | Drag a video, image or .kumo file here. |
| `lib.empty.button` | Datei auswählen | Choose a file |
| `lib.noresults` | Keine Treffer für „{q}“ | No results for "{q}" |
| `lib.noresults.clear` | Suche löschen | Clear search |
| `lib.drop.title` | Zum Hinzufügen loslassen | Drop to add |
| `lib.drop.formats` | MP4 · WebM · JPG · PNG · WebP · BMP · .kumo | MP4 · WebM · JPG · PNG · WebP · BMP · .kumo |
| `lib.drop.invalid` | Dieses Format wird nicht unterstützt | This format isn't supported |
| `lib.importing` | Wird hinzugefügt … | Adding… |
| `lib.converting` | Wird umgewandelt … {p} % | Converting… {p}% |
| `lib.err.format` | „{file}“ konnte nicht hinzugefügt werden. Dieses Format wird nicht unterstützt. | Couldn't add "{file}". This format isn't supported. |
| `lib.err.codec` | Für dieses Video ({codec}) fehlt die passende Videoerweiterung von Microsoft. | This video ({codec}) needs a video extension from Microsoft. |
| `lib.err.codec.button` | Im Store öffnen | Open in Store |
| `lib.warn.4k` | 4K-Video: Kumo braucht dafür mehr Grafikspeicher. Für die meisten Bildschirme reicht 1080p. | 4K video: Kumo needs more graphics memory for this. 1080p is enough for most displays. |
| `lib.missing` | Datei fehlt | File missing |
| `lib.missing.find` | Suchen … | Locate… |
| `lib.menu.applyto` | Anwenden auf | Apply to |
| `lib.menu.allscreens` | Alle Bildschirme | All displays |
| `lib.menu.screen` | Bildschirm {n} · {w} × {h} | Display {n} · {w} × {h} |
| `lib.menu.customize` | Anpassen … | Customize… |
| `lib.menu.explorer` | Im Explorer zeigen | Show in Explorer |
| `lib.menu.remove` | Aus Bibliothek entfernen | Remove from library |
| `lib.menu.remove.builtin` | Eingebaute Szenen können nicht entfernt werden | Built-in scenes can't be removed |
| `lib.remove.title` | „{name}“ entfernen? | Remove "{name}"? |
| `lib.remove.body` | Die Kopie in der Kumo-Bibliothek wird gelöscht. Deine Originaldatei bleibt erhalten. | The copy in the Kumo library will be deleted. Your original file is kept. |
| `lib.remove.active` | Bildschirm {n} zeigt danach „{other}“. | Display {n} will then show "{other}". |
| `lib.remove.confirm` | Entfernen | Remove |
| `lib.pick.banner` | Wähle einen Hintergrund für Bildschirm {n} | Choose a wallpaper for display {n} |
| `np.screens` | Läuft auf Bildschirm {list} · {fps} fps | Playing on display {list} · {fps} fps |
| `np.all` | Läuft auf allen Bildschirmen · {fps} fps | Playing on all displays · {fps} fps |
| `np.multi` | {n} Hintergründe aktiv | {n} wallpapers active |
| `np.paused` | Pausiert | Paused |
| `np.customize` | Anpassen | Customize |
| `np.pause` / `np.resume` | Pausieren / Fortsetzen | Pause / Resume |

#### Detail / customise

| Key | DE | EN |
|---|---|---|
| `detail.apply` | Anwenden | Apply |
| `detail.explorer` | Im Explorer zeigen | Show in Explorer |
| `detail.type` / `source` / `size` / `resolution` / `memory` | Typ / Quelle / Größe / Auflösung / Speicher | Type / Source / Size / Resolution / Memory |
| `detail.memory.scene` | ca. {a}–{b} MB gesamt | about {a}–{b} MB total |
| `detail.memory.video` | ca. +{mb} MB | about +{mb} MB |
| `detail.memory.image` | 0 MB – wird von Windows angezeigt | 0 MB – shown by Windows |
| `detail.memory.tip` | Schätzung. Die echte Zahl siehst du unter Wiedergabe & Leistung. | Estimate. See Playback & performance for the real number. |
| `detail.customize` | Anpassen | Customize |
| `detail.live` | Änderungen werden sofort auf dem Desktop übernommen. | Changes apply to your desktop right away. |
| `detail.inactive` | Dieser Hintergrund ist gerade nicht aktiv. Deine Einstellungen werden beim Anwenden verwendet. | This wallpaper isn't active right now. Your settings will be used when you apply it. |
| `detail.reset` | Auf Standard zurücksetzen | Reset to defaults |
| `detail.remove` | Aus Bibliothek entfernen | Remove from library |
| `prop.time` | Tageszeit | Time of day |
| `prop.time.real` / `fixed` / `lapse` | Echtzeit / Fest / Zeitraffer | Real time / Fixed / Time-lapse |
| `prop.time.real.desc` | Folgt deiner Uhr: Sonnenaufgang, Abendrot, Nacht. | Follows your clock: sunrise, sunset, night. |
| `prop.hour` | Uhrzeit | Time |
| `prop.hour.presets` | Morgen · Mittag · Abendrot · Nacht | Morning · Noon · Sunset · Night |
| `prop.lapse` | Geschwindigkeit | Speed |
| `prop.rotate` | Kamera dreht sich | Camera orbits |
| `prop.rotate.desc` | 0 = steht still | 0 = still |
| `prop.petals` | Kirschblüten | Cherry blossoms |
| `prop.petals.opts` | Keine / Wenige / Normal / Viele | None / Few / Normal / Many |
| `prop.parallax` | Parallaxe | Parallax |
| `prop.parallax.desc` | Die Kamera folgt leicht der Maus. | The camera follows the mouse a little. |
| `prop.fit` | Anpassung | Fit |
| `prop.fit.opts` | Füllen (zuschneiden) / Einpassen (Ränder) / Strecken / Zentriert / Kacheln | Fill (crop) / Fit (borders) / Stretch / Center / Tile |
| `prop.sound` | Ton abspielen | Play sound |
| `prop.sound.desc` | Nur hörbar, solange der Desktop sichtbar ist. | Only audible while the desktop is visible. |
| `prop.hw.ok` | Hardware-beschleunigt ({codec}) | Hardware accelerated ({codec}) |
| `prop.hw.no` | Ohne Hardware-Beschleunigung – höhere CPU-Last | No hardware acceleration – higher CPU load |
| `prop.image.info` | Bilder zeigt Windows selbst an – das kostet keinen zusätzlichen Speicher. | Windows shows images itself – this costs no extra memory. |
| `prop.web.warn` | Kompatibilitätsmodus: Web-Hintergründe nutzen einen eingebauten Browser und brauchen ca. 150–400 MB Arbeitsspeicher. | Compatibility mode: web wallpapers use a built-in browser and need about 150–400 MB of memory. |

#### Displays

| Key | DE | EN |
|---|---|---|
| `disp.title` | Bildschirme | Displays |
| `disp.identify` | Identifizieren | Identify |
| `disp.mode` | Hintergrund auf mehreren Bildschirmen | Wallpaper on multiple displays |
| `disp.mode.same` / `per` / `span` | Überall gleich / Pro Bildschirm / Strecken | Same on all / Per display / Span |
| `disp.single` | Schließe einen zweiten Bildschirm an, um ihm einen eigenen Hintergrund zu geben. | Connect a second display to give it its own wallpaper. |
| `disp.screen` | Bildschirm {n} | Display {n} |
| `disp.primary` | Hauptbildschirm | Main display |
| `disp.meta` | {name} · {w} × {h} · Skalierung {s} % | {name} · {w} × {h} · {s}% scale |
| `disp.wallpaper` | Hintergrund | Wallpaper |
| `disp.all` | Alle Bildschirme | All displays |
| `disp.change` | Ändern … | Change… |
| `disp.enabled` | Live-Hintergrund auf diesem Bildschirm | Live wallpaper on this display |
| `disp.enabled.off` | Windows zeigt hier dein normales Hintergrundbild. Spart Speicher und Strom. | Windows shows your normal wallpaper here. Saves memory and power. |
| `disp.enabled.last` | Mindestens ein Bildschirm bleibt an – zum Ausschalten „Pausieren“ nutzen oder Kumo beenden. | At least one display stays on – use "Pause" or quit Kumo to turn it off. |
| `disp.off` | Aus | Off |
| `disp.paused` | Pausiert | Paused |
| `disp.rotate` | Abwechseln | Rotate |
| `disp.rotate.opts` | Nie / Alle 15 min / Alle 30 min / Jede Stunde / Nach Tageszeit … | Never / Every 15 min / Every 30 min / Every hour / By time of day… |
| `disp.mem` | Unterschiedliche Hintergründe brauchen mehr Speicher – gerade {now} MB statt {same} MB. Gleiche Hintergründe auf Bildschirmen mit gleichem Seitenverhältnis teilen sich ein Bild. | Different wallpapers need more memory – {now} MB right now instead of {same} MB. Displays with the same aspect ratio share one picture when they show the same wallpaper. |
| `disp.new` | Neuer Bildschirm erkannt. Er zeigt denselben Hintergrund wie Bildschirm {n}. | New display detected. It shows the same wallpaper as display {n}. |

#### Playback & performance

| Key | DE | EN |
|---|---|---|
| `perf.title` | Wiedergabe & Leistung | Playback & performance |
| `perf.ram` | Arbeitsspeicher | Memory |
| `perf.gpu` / `perf.cpu` | GPU / CPU | GPU / CPU |
| `perf.state` | {n} Bildschirme · {fps} fps | {n} displays · {fps} fps |
| `perf.panel` | Dieses Fenster braucht zusätzlich ca. {mb} MB und wird beim Schließen komplett beendet. | This window uses about {mb} MB more and closes completely when you close it. |
| `perf.pause` / `perf.resume` | Pausieren / Fortsetzen | Pause / Resume |
| `perf.quality` | Bildqualität | Picture quality |
| `perf.fps` | Bildrate | Frame rate |
| `perf.fps.desc` | 30 fps wirkt bei Hintergründen flüssig und braucht etwa halb so viel Grafikleistung wie 60. | 30 fps looks smooth for wallpapers and needs about half the graphics power of 60. |
| `perf.scale` | Renderauflösung | Render resolution |
| `perf.scale.opts` | Hoch (100 %) / Ausgewogen (80 %) / Sparsam (62 %) | High (100%) / Balanced (80%) / Light (62%) |
| `perf.scale.desc` | Niedriger spart Grafikleistung, das Bild wird etwas weicher. | Lower saves graphics power; the picture gets a little softer. |
| `perf.advanced` | Erweitert | Advanced |
| `perf.fxaa` | Kantenglättung (FXAA) | Edge smoothing (FXAA) |
| `perf.fxaa.desc` | Glättet Treppenstufen an Kanten. Kostet sehr wenig. | Smooths jagged edges. Costs very little. |
| `perf.rules` | Automatisch pausieren | Pause automatically |
| `perf.rules.tip` | Pausiert: Das letzte Bild bleibt stehen, Kumo braucht dann 0 % CPU und GPU. Standbild: Kumo gibt zusätzlich seinen Grafikspeicher frei und zeigt ein Foto der Szene als Windows-Hintergrund. | Paused: the last frame stays, Kumo uses 0% CPU and GPU. Still image: Kumo also frees its graphics memory and shows a photo of the scene as your Windows wallpaper. |
| `perf.covered` | Wenn Fenster den Bildschirm verdecken | When windows cover the display |
| `perf.covered.desc` | Ein verdeckter Bildschirm wird nicht gezeichnet. | A covered display isn't drawn. |
| `perf.covered.opts` | Bei Vollbild und maximierten Fenstern (empfohlen) / Nur bei Vollbild / Nie | Full screen and maximized windows (recommended) / Full screen only / Never |
| `perf.battery` | Im Akkubetrieb | On battery |
| `perf.saver` | Mit Energiesparmodus | With energy saver |
| `perf.power.opts` | Normal weiterlaufen / Bildrate halbieren / Pausieren / Standbild (Speicher freigeben) | Keep running / Half frame rate / Pause / Still image (free memory) |
| `perf.unfocused` | Nur auf dem freien Desktop abspielen | Play only on the empty desktop |
| `perf.unfocused.desc` | Pausiert, sobald ein anderes Fenster aktiv ist. | Pauses as soon as another window is active. |
| `perf.scope` | Pausieren gilt für | Pausing applies to |
| `perf.scope.opts` | Nur den verdeckten Bildschirm / Alle Bildschirme | Only the covered display / All displays |
| `perf.more` | Weitere Regeln | More rules |
| `perf.apps` | App-Regeln | App rules |
| `perf.apps.desc` | Pausiert, solange bestimmte Programme laufen – z. B. Spiele im Fenstermodus. | Pauses while certain programs run – e.g. games in windowed mode. |
| `perf.apps.add` | App hinzufügen … | Add app… |
| `perf.always` | Immer pausiert: gesperrter PC, ausgeschalteter Bildschirm und Remotedesktop-Sitzungen. | Always paused: locked PC, display off and Remote Desktop sessions. |

#### Settings & About

| Key | DE | EN |
|---|---|---|
| `set.title` | Einstellungen | Settings |
| `set.general` | Allgemein | General |
| `set.autostart` | Mit Windows starten | Start with Windows |
| `set.autostart.desc` | Kumo startet nach der Anmeldung unauffällig im Hintergrund. | Kumo starts quietly in the background when you sign in. |
| `set.lang` | Sprache | Language |
| `set.lang.desc` | Gilt für dieses Fenster und das Menü im Infobereich. | Applies to this window and the notification area menu. |
| `set.lang.opts` | Wie Windows / Deutsch / English | Same as Windows / Deutsch / English |
| `set.theme` | Farbmodus | Color mode |
| `set.theme.opts` | Wie Windows / Hell / Dunkel | Same as Windows / Light / Dark |
| `set.library` | Bibliothek | Library |
| `set.copy` | Dateien beim Hinzufügen kopieren | Copy files when adding |
| `set.copy.desc` | Der Hintergrund funktioniert dann auch, wenn du die Originaldatei verschiebst oder löschst. | The wallpaper keeps working even if you move or delete the original file. |
| `set.location` | Speicherort | Location |
| `set.used` | Belegt: {size} | Used: {size} |
| `set.openfolder` | Ordner öffnen | Open folder |
| `set.quit` | Beim Beenden | When quitting |
| `set.quit.wall` | Windows-Hintergrund nach dem Beenden | Windows wallpaper after quitting |
| `set.quit.opts` | Vorheriges Bild wiederherstellen / Standbild des Live-Hintergrunds | Restore previous picture / Still of the live wallpaper |
| `set.trouble` | Problembehandlung | Troubleshooting |
| `set.log` | Protokolldatei | Log file |
| `set.log.desc` | Hilfreich für Fehlerberichte. | Useful for bug reports. |
| `set.settingsfolder` | Einstellungsordner | Settings folder |
| `set.restart` | Kumo neu starten | Restart Kumo |
| `set.restart.desc` | Startet den Hintergrund neu, ohne dass du dich abmelden musst. | Restarts the wallpaper without signing out. |
| `set.restart.button` | Neu starten | Restart |
| `set.reset` | Alle Einstellungen zurücksetzen | Reset all settings |
| `set.reset.button` | Zurücksetzen … | Reset… |
| `set.reset.title` | Alle Einstellungen zurücksetzen? | Reset all settings? |
| `set.reset.body` | Bildrate, Qualität, Pausenregeln und die Zuordnung der Bildschirme werden auf den Standard gesetzt. Deine Bibliothek bleibt erhalten. | Frame rate, quality, pause rules and display assignments go back to their defaults. Your library is kept. |
| `about.tagline` | Leichtgewichtige Live-Hintergründe für Windows. 雲 (kumo) ist Japanisch für „Wolke“. | Lightweight live wallpapers for Windows. 雲 (kumo) is Japanese for "cloud". |
| `about.version` | Version {v} | Version {v} |
| `about.update` | Nach Updates suchen | Check for updates |
| `about.update.checking` | Suche nach Updates … | Checking for updates… |
| `about.update.none` | Kumo ist auf dem neuesten Stand. | Kumo is up to date. |
| `about.update.available` | Version {v} herunterladen | Download version {v} |
| `about.update.error` | Keine Verbindung zu GitHub. | Couldn't reach GitHub. |
| `about.update.note` | Kumo prüft nur, wenn du hier klickst. | Kumo only checks when you click here. |
| `about.source` | Quellcode auf GitHub | Source code on GitHub |
| `about.bug` | Problem melden | Report a problem |
| `about.bug.desc` | Hänge am besten die Protokolldatei an. | Attaching the log file helps. |
| `about.license` | Lizenz | License |
| `about.thirdparty` | Hinweise zu Drittanbieter-Software | Third-party notices |
| `about.privacy` | Kein Konto, keine Werbung, keine Telemetrie. | No account, no ads, no telemetry. |
| `about.uninstall` | Kumo deinstallieren … | Uninstall Kumo… |

#### Global InfoBars

| Key | DE | EN |
|---|---|---|
| `g.stopped` | Kumo läuft gerade nicht – dein Live-Hintergrund ist aus. | Kumo isn't running – your live wallpaper is off. |
| `g.stopped.button` | Kumo starten | Start Kumo |
| `g.denied` | Kumo konnte sich nicht hinter die Desktop-Symbole legen. | Kumo couldn't place itself behind the desktop icons. |
| `g.denied.details` / `g.retry` | Details / Erneut versuchen | Details / Try again |
| `g.conflict` | {app} läuft ebenfalls. Zwei Hintergrund-Programme stören sich gegenseitig – beende {app} über sein Symbol im Infobereich. | {app} is running too. Two wallpaper programs get in each other's way – quit {app} from its notification area icon. |
| `g.conflict.button` | Erneut prüfen | Check again |
| `g.firstrun.tip` | Kumo läuft im Hintergrund weiter, auch wenn du dieses Fenster schließt. Du findest es unten rechts im Infobereich (Kirschblüte). | Kumo keeps running in the background when you close this window. You'll find it in the notification area at the bottom right (cherry blossom). |

#### Tray

| Key | DE | EN |
|---|---|---|
| `tray.open` | Kumo öffnen | Open Kumo |
| `tray.pause` / `tray.resume` | Pausieren / Fortsetzen | Pause / Resume |
| `tray.fps` | Bildrate | Frame rate |
| `tray.quit` | Beenden | Quit |
| `tray.tooltip` | Kumo – {name} | Kumo – {name} |
| `tray.tooltip.paused` | Kumo – pausiert | Kumo – paused |

#### Setup wizard

| Key | DE | EN |
|---|---|---|
| `w.window` | Kumo installieren | Install Kumo |
| `w.step` | Schritt {n} von 3 | Step {n} of 3 |
| `w.art.sub` | Live-Hintergründe | Live wallpapers |
| `w.next` / `w.back` / `w.cancel` | Weiter / Zurück / Abbrechen | Next / Back / Cancel |
| `w1.title` | Willkommen bei Kumo | Welcome to Kumo |
| `w1.lead` | Kumo zeigt animierte 3D-Szenen, Videos und Bilder als Desktop-Hintergrund – mit einem Bruchteil des Arbeitsspeichers. | Kumo shows animated 3D scenes, videos and pictures as your desktop wallpaper – using a fraction of the memory. |
| `w1.p1` | Rund 70 MB statt mehrerer hundert MB | About 70 MB instead of several hundred |
| `w1.p2` | Pausiert von selbst, wenn du arbeitest oder spielst | Pauses by itself while you work or play |
| `w1.p3` | Kostenlos, Open Source, ohne Konto und Werbung | Free, open source, no account, no ads |
| `w1.conflict` | {app} läuft gerade. Beende es vor der Installation, damit sich beide Programme nicht stören. | {app} is running. Quit it before installing so the two programs don't get in each other's way. |
| `w2.title` | So wird Kumo eingerichtet | How Kumo will be set up |
| `w2.autostart` | Mit Windows starten | Start with Windows |
| `w2.autostart.desc` | Empfohlen. Sonst musst du Kumo nach jedem Neustart selbst öffnen. | Recommended. Otherwise you have to open Kumo yourself after every restart. |
| `w2.desktop` | Verknüpfung auf dem Desktop | Desktop shortcut |
| `w2.folder` | Installationsordner: C:\Program Files\Kumo | Install folder: C:\Program Files\Kumo |
| `w2.folder.desc` | Fest vorgegeben: Windows erlaubt Live-Hintergründe nur für Programme aus diesem Ordner. | Fixed: Windows only allows live wallpapers for programs in this folder. |
| `w2.startmenu` | Startmenü-Eintrag und Eintrag unter Einstellungen › Apps werden immer angelegt. | A Start menu entry and an entry in Settings › Apps are always created. |
| `w2.uac` | Windows fragt gleich einmal nach Administratorrechten. | Windows will ask for administrator rights once. |
| `w2.size` | Benötigt ca. 6 MB | Needs about 6 MB |
| `w2.install` | Installieren | Install |
| `w3.title` | Kumo wird installiert … | Installing Kumo… |
| `w3.uac` | Bitte bestätige die Abfrage von Windows. | Please confirm the Windows prompt. |
| `w3.s1` / `s2` / `s3` / `s4` | Dateien kopieren / Startmenü-Eintrag anlegen / Autostart einrichten / Bei „Apps“ eintragen | Copy files / Create Start menu entry / Set up autostart / Register in "Apps" |
| `w3.err.title` | Installation nicht abgeschlossen | Installation didn't finish |
| `w3.err.uac` | Ohne Administratorrechte kann Kumo nicht in „Programme“ installiert werden. | Kumo can't be installed to Program Files without administrator rights. |
| `w3.err.copy` | Eine Datei konnte nicht geschrieben werden. | A file couldn't be written. |
| `w3.retry` | Erneut versuchen | Try again |
| `w4a.title` | Fast geschafft! | Almost done! |
| `w4a.body` | Starte Kumo jetzt einmal über das Startmenü: | Now start Kumo once from the Start menu: |
| `w4a.s1` | Drücke die Windows-Taste ⊞ | Press the Windows key ⊞ |
| `w4a.s2` | Tippe „Kumo“ | Type "Kumo" |
| `w4a.s3` | Drücke die Eingabetaste | Press Enter |
| `w4a.why` | Warum kann ich Kumo nicht hier starten? | Why can't I start Kumo from here? |
| `w4a.why.body` | Windows lässt Live-Hintergründe nur von Programmen aus dem Ordner „Programme“ starten – nicht von dieser heruntergeladenen Datei. Ab der nächsten Anmeldung startet Kumo automatisch. | Windows only lets programs in Program Files run live wallpapers – not this downloaded file. From your next sign-in, Kumo starts automatically. |
| `w4a.waiting` | Warte auf Kumo … | Waiting for Kumo… |
| `w4b.title` | Kumo ist installiert | Kumo is installed |
| `w4b.body` | Kumo startet ab jetzt automatisch mit Windows. | From now on, Kumo starts automatically with Windows. |
| `w4b.start` | Kumo jetzt starten | Start Kumo now |
| `w4b.starting` | Kumo wird gestartet … | Starting Kumo… |
| `w4b.fallback` | Windows hat den Start von hier blockiert. Kein Problem – starte Kumo über das Startmenü: | Windows blocked starting Kumo from here. No problem – start it from the Start menu: |
| `w5.title` | Kumo läuft! | Kumo is running! |
| `w5.body` | Dein neuer Hintergrund ist da. Die Kirschblüte unten rechts im Infobereich öffnet Kumo – auch später jederzeit. | Your new wallpaper is here. The cherry blossom in the notification area opens Kumo – any time. |
| `w5.cleanup` | Die heruntergeladene Datei brauchst du nicht mehr und kannst sie löschen. | You no longer need the downloaded file and can delete it. |
| `w5.open` | Kumo öffnen | Open Kumo |
| `u.title` | Kumo aktualisieren | Update Kumo |
| `u.keep` | Deine Einstellungen und deine Bibliothek bleiben erhalten. | Your settings and library are kept. |
| `u.whatsnew` | Was ist neu? | What's new? |
| `u.button` | Aktualisieren | Update |
| `u.s1` / `u.s2` / `u.s3` | Kumo kurz beenden (dein Hintergrund ist kurz normal) / Dateien ersetzen / Kumo neu starten | Briefly quit Kumo (your wallpaper is normal for a moment) / Replace files / Restart Kumo |
| `u.done` | Kumo ist auf dem neuesten Stand. | Kumo is up to date. |
| `r.title` | Kumo {v} ist bereits installiert | Kumo {v} is already installed |
| `r.repair` | Reparieren | Repair |
| `r.repair.desc` | Dateien neu kopieren. Einstellungen bleiben. | Copy the files again. Settings are kept. |
| `r.uninstall` | Deinstallieren | Uninstall |
| `r.older` | Installiert ist eine neuere Version ({v}). Diese Datei ist älter. | A newer version ({v}) is installed. This file is older. |
| `x.title` | Kumo deinstallieren? | Uninstall Kumo? |
| `x.body` | Kumo wird beendet und entfernt. Dein vorheriges Windows-Hintergrundbild kommt sofort zurück. | Kumo will quit and be removed. Your previous Windows wallpaper comes back right away. |
| `x.data` | Auch meine Einstellungen und Bibliothek löschen ({size}) | Also delete my settings and library ({size}) |
| `x.button` | Deinstallieren | Uninstall |
| `x.done` | Kumo wurde entfernt. | Kumo has been removed. |

---

## 11. Acceptance checks

* Panel open on the library with 20 items: private working set ≤ 40 MB (fail > 60). Idle CPU 0.0 % after 5 s. After closing, no `kumo.exe --panel` process is left.
* Engine RAM with the panel closed is unchanged against v0.1 (same scene, same screens).
* German UI at 100 % and 150 % scale, 720×520 window: no clipped text, no horizontal scrolling.
* All flows work by keyboard only (Tab, arrows, Enter, Esc).
* Drag & drop from Explorer works (panel not elevated).
* The wizard on a clean Windows 11 24H2 VM: install → W4-A → start from the Start menu → W5 shown automatically within 1 s.
* Update 0.1 → 0.2 while the engine runs: the wallpaper comes back without the Start menu step.
* Uninstall with "keep data" off restores the original Windows wallpaper and removes `%LOCALAPPDATA%\Kumo` and `%APPDATA%\Kumo`.

## 12. Open questions for the owner

1. Energy saver default: keep "half frame rate" (today) or switch to "pause"?
2. Should the tray keep "Tageszeit ▸" for quick access, or only the four items?
3. Does the "Kumo öffnen" panel live in the same exe (`--panel`, recommended) or in a separate `kumo-panel.exe`?
4. License choice is needed before the About page links can be final.

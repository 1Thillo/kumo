# Kumo im Microsoft Store

Alles, was für die Einreichung im Partner Center gebraucht wird. Die Texte sind zum Kopieren.

## Ablauf

1. **Konto:** Unter [storedeveloper.microsoft.com](https://storedeveloper.microsoft.com) als **Company** registrieren, mit `hello@hyos.tech`. Nicht als Individual: Das lässt sich später nicht ändern und ist nicht für gewerblichen Verkauf gedacht. Für die Prüfung braucht Microsoft eine DUNS-Nummer oder ein offizielles Dokument, vermutlich reicht der Gewerbeschein.
2. **Name reservieren:** *Apps and games → New product → MSIX or PWA app*, Name `Kumo`. Ist er vergeben, wäre `Kumo Live Wallpaper` die Alternative.
3. **Identität übernehmen:** Unter *Product management → Product identity* stehen drei Werte: `Package/Identity/Name`, `Package/Identity/Publisher`, `Package/Properties/PublisherDisplayName`.
4. **Paket bauen** mit genau diesen Werten:

   ```powershell
   .\tools\package_msix.ps1 -Name "<Identity/Name>" -Publisher "<Identity/Publisher>" -PublisherName "<PublisherDisplayName>"
   ```

5. **Einreichung ausfüllen** mit den Angaben unten und das `.msix` hochladen.

Die Version im Store muss mit mindestens 1 beginnen (1.0.0.0). Das Paket muss nicht signiert sein, der Store signiert es selbst.

## Angaben für die Einreichung

| Feld | Wert |
|---|---|
| Kategorie | Personalization, Unterkategorie Wallpaper & lock screens |
| Preis | 2,99 € (Base price, alle Märkte nach deiner Wahl) |
| Kostenlose Testversion | Keine: Die kostenlose Version auf GitHub erfüllt diesen Zweck |
| Website | https://kumo.hyos.tech |
| Support-Kontakt | hello@hyos.tech |
| Datenschutzerklärung | https://kumo.hyos.tech/datenschutz |
| Copyright | © 2026 Hamid Yosefsei |
| Lizenzbedingungen | Eigene Lizenz: GPL-3.0 (siehe unten), nicht die Standardbedingungen von Microsoft |
| Altersfreigabe | Fragebogen im Partner Center ehrlich beantworten: keine Gewalt, keine Käufe in der App, keine Kommunikation, keine Daten |
| Barrierefreiheit | Nicht ankreuzen, solange das nicht nach Microsofts Richtlinien getestet ist |

### Zusätzliche Lizenzbedingungen

> Kumo is free software under the GNU General Public License v3.0. You may use, study, change and share it under the terms of that license: https://www.gnu.org/licenses/gpl-3.0.html. The complete source code is available at https://github.com/Hamido212/kumo. Kumo comes with no warranty.

### Begründung für runFullTrust

Das Feld erscheint bei *Submission options*, weil Kumo eine eingeschränkte Fähigkeit nutzt:

> Kumo is a Win32 live wallpaper engine written in Rust. It needs full trust to place a Direct3D 11 window behind the desktop icons, which requires re-parenting into the desktop window hierarchy, and to show a notification area icon, pause when windows cover the desktop and decode video through Media Foundation. None of this is possible in an app container. Kumo has no network access, no accounts and no telemetry.

### Hinweise für die Prüfung

> Kumo draws a live wallpaper behind the desktop icons as soon as it starts, and a control panel opens on the first start. It then keeps running with an icon in the notification area: right-click it to pause or quit. To test it, drag any MP4 or picture into the control panel. No account or test data is needed.

## Store-Eintrag: Deutsch

**Kurzbeschreibung**

> Live-Hintergründe für Windows, die deinen PC in Ruhe lassen: 3D-Szenen, Videos und Bilder hinter deinen Desktop-Symbolen, ohne Browser im Hintergrund.

**Beschreibung**

> Kumo (雲, japanisch für Wolke) bringt Leben auf deinen Desktop, ohne deinen PC auszubremsen.
>
> Die meisten Live-Hintergründe laufen in einem eingebetteten Browser, oft einem pro Bildschirm. Kumo zeichnet direkt mit der Grafikkarte. Mitgeliefert wird eine schwebende Voxel-Insel mit fünfstöckiger Pagode, fallenden Kirschblüten, Koi-Teich, Wasserfall und einem echten Tag-und-Nacht-Wechsel nach deiner Uhr.
>
> Eigene Videos und Bilder ziehst du einfach ins Fenster. Du entscheidest, ob jeder Bildschirm dasselbe zeigt, etwas anderes oder gar nichts. Kumo pausiert von selbst, wenn ein Fenster den Bildschirm verdeckt, auf Akku, im Energiesparmodus oder bei gesperrtem PC.
>
> Kein Konto, keine Werbung, keine Telemetrie. Kumo ist Open Source unter der GPL-3.0, den Quellcode findest du auf GitHub. Mit dem Kauf im Store bekommst du automatische Updates und unterstützt die Weiterentwicklung.

**Funktionen**

> 3D-Szenen, Videos und Bilder als Hintergrund
> Eigener Hintergrund pro Bildschirm
> Tageszeit nach deiner Uhr, fest oder im Zeitraffer
> Pausiert von selbst bei verdecktem Bildschirm, Akku und gesperrtem PC
> 15, 30 oder 60 Bilder pro Sekunde
> Kein Konto, keine Werbung, keine Telemetrie
> Deutsch und Englisch, hell und dunkel

**Suchbegriffe:** Live-Hintergrund, Wallpaper, animierter Hintergrund, Desktop, Video-Hintergrund, Live Wallpaper, 3D

## Store listing: English

**Short description**

> Live wallpapers for Windows that leave your PC alone: 3D scenes, videos and pictures behind your desktop icons, with no browser in the background.

**Description**

> Kumo (雲, Japanese for cloud) brings your desktop to life without slowing your PC down.
>
> Most live wallpapers run inside an embedded browser, often one per screen. Kumo draws directly with the graphics card. It comes with a floating voxel island: a five-storey pagoda, falling cherry blossoms, a koi pond, a waterfall and a real day and night that follow your clock.
>
> Drag your own videos and pictures into the window. Show the same wallpaper everywhere, a different one per screen, or none on a screen. Kumo pauses by itself when a window covers the screen, on battery, in energy saver mode or on a locked PC.
>
> No account, no ads, no telemetry. Kumo is open source under the GPL-3.0, and the source code is on GitHub. Buying it in the Store gets you automatic updates and supports its development.

**Features**

> 3D scenes, videos and pictures as your wallpaper
> A different wallpaper per screen
> Time of day from your clock, fixed or as a time-lapse
> Pauses by itself on covered screens, on battery and on a locked PC
> 15, 30 or 60 frames per second
> No account, no ads, no telemetry
> German and English, light and dark

**Search terms:** live wallpaper, animated wallpaper, desktop, video wallpaper, lightweight, 3D, background

## Bilder

Alle liegen in `package/store` und entstehen mit `tools/store_images.ps1`.

| Feld im Partner Center | Datei |
|---|---|
| Screenshots | `screenshot-evening`, `-night`, `-noon`, dazu `-screens-*` und `-library-*` für die jeweilige Sprache |
| 1:1 Box art | `box-1080.png` oder `box-2160.png` |
| 2:3 Poster art | `poster-720x1080.png` oder `poster-1440x2160.png` |
| 1:1 App tile icon | `icon-300.png` |

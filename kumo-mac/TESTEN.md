# Kumo auf dem Mac testen

Danke fürs Testen! Das ist eine frühe Testversion. Sie zeigt nur die eingebaute Szene und hat noch
kein Bedienfeld. Sie läuft auf Macs mit Apple-Chip und Intel, ab macOS 11.

## Installieren

1. `Kumo-mac.zip` entpacken und `Kumo.app` in den Ordner **Programme** ziehen.
2. `Kumo.app` öffnen. macOS warnt jetzt, weil die Testversion noch nicht von Apple geprüft ist.
   Klick auf **Fertig**, nicht auf „In den Papierkorb“.
3. **Systemeinstellungen > Datenschutz & Sicherheit** öffnen, ganz nach unten scrollen und bei Kumo
   auf **Trotzdem öffnen** klicken. Danach mit deinem Passwort bestätigen.

Kumo hat kein Fenster und kein Dock-Symbol. Du erkennst es am Sakura-Symbol oben in der Menüleiste.

## Bitte prüfen

Schreib bei jedem Punkt kurz, ob es klappt:

1. **Hintergrund:** Läuft die Pagoden-Szene hinter den Desktop-Symbolen? Lassen sich die Symbole
   normal anklicken?
2. **Bild:** Ist das Bild scharf genug? Sehen die Farben normal aus, oder gibt es Streifen oder
   Flackern?
3. **Menüleiste:** Klick aufs Sakura-Symbol. Funktionieren **Pausieren**, **Fortsetzen** und
   **Kumo beenden**?
4. **Schreibtische:** Bleibt der Hintergrund beim Wechseln der Schreibtische sichtbar, auch in
   Mission Control?
5. **Vollbild:** Öffne eine App im Vollbild. Bleibt danach alles flüssig?
6. **Zweiter Bildschirm**, falls vorhanden: Läuft Kumo auf beiden? Und wenn du ihn abziehst?
7. **Ruhezustand:** Läuft Kumo nach dem Zuklappen und Aufklappen normal weiter?
8. **Akku:** Wird der Mac spürbar warm oder laut?

## Speicher messen

Öffne die **Aktivitätsanzeige** und suche nach `kumo`. Notiere bitte:

- die Werte bei **Speicher** und **% CPU**, während der Hintergrund sichtbar ist,
- dieselben Werte, während eine App im Vollbild läuft.

## Zurückschicken

- Welcher Mac: Modell, Chip, Arbeitsspeicher und macOS-Version
- Deine Antworten und die Messwerte
- Bei Fehlern ein Foto oder Video vom Bildschirm
- Die Protokolldatei. Dafür im Finder **Gehe zu > Gehe zum Ordner** wählen und
  `~/Library/Application Support/Kumo` eingeben. Darin liegt `kumo.log`.

## Entfernen

Kumo über das Menüleisten-Symbol beenden und `Kumo.app` in den Papierkorb legen. Den Ordner
`~/Library/Application Support/Kumo` kannst du auch löschen.

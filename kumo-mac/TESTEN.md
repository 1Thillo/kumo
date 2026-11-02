# Kumo auf dem Mac testen

Danke fürs Testen! Das ist eine frühe Testversion mit Bedienfeld, eigenen Videos und Bildern. Sie
läuft auf Macs mit Apple-Chip und Intel, ab macOS 11.

## Installieren

1. `Kumo-mac.zip` entpacken und `Kumo.app` in den Ordner **Programme** ziehen.
2. `Kumo.app` öffnen. macOS warnt jetzt, weil die Testversion noch nicht von Apple geprüft ist.
   Klick auf **Fertig**, nicht auf „In den Papierkorb“.
3. **Systemeinstellungen > Datenschutz & Sicherheit** öffnen, ganz nach unten scrollen und bei Kumo
   auf **Trotzdem öffnen** klicken. Danach mit deinem Passwort bestätigen.

Beim ersten Start öffnet sich das Kumo-Fenster und fragt nach dem Start beim Anmelden. Später
erreichst du Kumo über das Sakura-Symbol oben in der Menüleiste.

## Bitte prüfen

Schreib bei jedem Punkt kurz, ob es klappt:

1. **Hintergrund:** Läuft die Pagoden-Szene hinter den Desktop-Symbolen? Lassen sich die Symbole
   normal anklicken?
2. **Bild:** Ist das Bild scharf genug? Sehen die Farben normal aus, oder gibt es Streifen oder
   Flackern?
3. **Menüleiste:** Klick aufs Sakura-Symbol. Funktionieren **Kumo öffnen**, **Pausieren**,
   **Fortsetzen** und **Kumo beenden**?
4. **Eigenes Video:** Zieh ein MP4- oder MOV-Video ins Kumo-Fenster. Erscheint ein Vorschaubild?
   Läuft das Video als Hintergrund, wenn du es anwendest, und beginnt es am Ende nahtlos neu?
5. **Eigenes Bild:** Dasselbe mit einem Foto.
6. **Anpassen:** Klick bei einem Video oder Bild auf **Anpassen**. Ändern sich Füllung und
   Wiedergabetempo sofort? Klick bei der Pagode auf **Anpassen** und ändere die Tageszeit.
7. **Einstellungen:** Wechsle Sprache und Farbmodus. Sind alle Symbole sichtbar, keine leeren
   Kästchen? Stimmt der Text überall, und taucht irgendwo noch „Windows“ auf?
8. **Autostart:** Schalte **Beim Anmelden starten** ein, melde dich ab und wieder an. Ist Kumo
   wieder da?
9. **Schreibtische:** Bleibt der Hintergrund beim Wechseln der Schreibtische sichtbar, auch in
   Mission Control?
10. **Vollbild:** Öffne eine App im Vollbild. Bleibt danach alles flüssig?
11. **Zweiter Bildschirm**, falls vorhanden: Läuft Kumo auf beiden? Kannst du unter **Bildschirme**
    jedem Bildschirm einen eigenen Hintergrund geben? Und wenn du einen abziehst?
12. **Ruhezustand:** Läuft Kumo nach dem Zuklappen und Aufklappen normal weiter?
13. **Akku:** Wird der Mac spürbar warm oder laut?

## Speicher messen

Öffne die **Aktivitätsanzeige** und suche nach `kumo`. Notiere bitte **Speicher** und **% CPU**:

- mit der Pagoden-Szene, während der Hintergrund sichtbar ist,
- mit der Pagoden-Szene, während eine App im Vollbild läuft,
- mit deinem Video.

Zwei Einträge namens `kumo` sind normal, solange das Kumo-Fenster offen ist. Gemeint ist der
Eintrag, der nach dem Schließen des Fensters bleibt.

## Zurückschicken

- Welcher Mac: Modell, Chip, Arbeitsspeicher und macOS-Version
- Deine Antworten und die Messwerte
- Bei Fehlern ein Foto oder Video vom Bildschirm
- Die Protokolldatei. Dafür im Finder **Gehe zu > Gehe zum Ordner** wählen und
  `~/Library/Application Support/Kumo` eingeben. Darin liegt `kumo.log`.

## Entfernen

1. Im Kumo-Fenster **Beim Anmelden starten** ausschalten.
2. Kumo über das Menüleisten-Symbol beenden und `Kumo.app` in den Papierkorb legen.
3. Den Ordner `~/Library/Application Support/Kumo` löschen. Darin liegen auch die Kopien deiner
   Videos und Bilder.

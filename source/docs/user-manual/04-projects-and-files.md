# 4 Projekte und Dateien

[Zurück](03-interface-overview.md) · [Inhaltsverzeichnis](README.md) · [Weiter: Editor](05-editor.md)

Ein **Projekt** ist ein Ordner. Es gibt keinen „Neues Projekt“-Assistenten.

## Projekt anlegen

1. Ordner im Windows-Explorer anlegen.
2. Optional eine `nortex.toml` darin ablegen (Manifest; siehe [Manifest-Schema](../build-system/manifest.md)).
3. Nortex Code mit diesem Ordner als Arbeitsordner starten, oder den Pfad in das Feld oben im Explorer eintragen und **Öffnen** klicken.

## Projekt öffnen / aktualisieren

Pfad im Explorer-Feld eingeben → **Öffnen**. Meldung „Projekt geladen“ oder „Projekt konnte nicht aktualisiert werden: …“ (Statusleiste). **Datei → Projekt aktualisieren** liest die Dateiliste im Hintergrund neu (auch nach Änderungen außerhalb der IDE). Mit einer `nortex.toml` im Projektordner zeigt die Statusleiste „Name → Ausgabedatei“ und der Fenstertitel lautet „<Name> — NORTEX Code“.

## Dateien

| Aktion | Vorgehen |
|---|---|
| Neu | **+** im Tab-Bereich, *Datei → Neue Datei* oder *Neu* im Dashboard |
| Öffnen | Klick im Explorer · Strg+P (Windows-Dateidialog, Filter `.nx`/`.nortex`) · *Datei öffnen* im Dashboard · Datei auf das Fenster ziehen · zuletzt geöffnete Datei im Dashboard |
| Speichern | Strg+S oder **Speichern**. Bei einem neuen Dokument öffnet sich der Speichern-Dialog. |
| Speichern unter | *Datei → Speichern unter…* oder Strg+Umschalt+S öffnet den Dateidialog. |
| Schließen | **×** im Tab. Bei ungespeicherten Änderungen kann gespeichert, verworfen oder abgebrochen werden. |

Gespeichert wird atomar (ein fehlgeschlagener Schreibvorgang lässt die Originaldatei unverändert). Fehler erscheinen als Statusmeldung, z. B. „Speichern fehlgeschlagen: …“.

## Nicht vorhanden

Umbenennen, Löschen, Ordner anlegen und Verschieben von Dateien gehen nur außerhalb der IDE (Windows-Explorer, Terminal-Tab). Danach *Projekt aktualisieren*. Der Explorer zeigt alle Dateien (außer `.git`) als Baum.

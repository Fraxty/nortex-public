# 13 FAQ und Fehlerbehebung

[Zurück](12-efficient-workflows.md) · [Inhaltsverzeichnis](README.md)

**Warum lässt sich ein Projekt nicht öffnen?**
Pfad prüfen (Ordner muss existieren). Drag & Drop nimmt nur `.nx` an („Nur .nx-Dateien werden geöffnet“). Ein defektes `nortex.toml` erzeugt eine Meldung in der Statusleiste.

**Warum funktioniert die Kompilierung nicht?**
1. „Compiler … konnte nicht gestartet werden“: `nortexc.exe` muss neben `nortex-code.exe` liegen oder im PATH sein.
2. Linker-/Runtime-Fehler: `nortex_runtime.lib` fehlt (neben der EXE, im Elternordner oder über `NORTEX_RUNTIME_LIB`).
3. Neue, noch nicht gespeicherte Dateien: erst mit Strg+S speichern und im Dateidialog einen Namen wählen.
4. NX-Fehler: siehe [Kapitel 8](08-debugging.md).

**Wo finde ich Fehlermeldungen?** Im Tab-Reiter *Compiler Log* (Strg+Umschalt+M), im Editor markiert, Anzahl in der Statusleiste.

**Wie öffne ich das Terminal?** Reiter *Terminal* im unteren Panel; Panel mit Strg+J einblenden.

**Wie speichere ich Dateien?** Strg+S oder *Datei → Speichern*; Strg+Umschalt+S für „Speichern unter“. Ungespeicherte Tabs zeigen •.

**Wie ändere ich die Darstellung?** Zahnrad → Theme, Dichte, Zoom ([Kapitel 9](09-settings-and-customization.md)).

**Mehrere Dateien gleichzeitig bearbeiten?** Tabs und geteilter Editor ([Kapitel 5](05-editor.md)).

**Warum zeigt der Run-Button „Exitcode Some(0)“?** So gibt die IDE den Exitcode 0 aus; kein Fehler.

**Warum meldet die IDE bei `float` oder `struct` einen Fehler?** Diese Konstrukte sind noch nicht nativ erzeugbar (NX2001/NX2002).

**Gibt es Debugger, Git-Commit, Umbenennen/Löschen, KI?** Nein, siehe Funktionsstand im [Inhaltsverzeichnis](README.md).

**Die Einstellungen sind zurückgesetzt.** `%APPDATA%\NORTEX Code\settings.json` löschen; sie wird neu angelegt.

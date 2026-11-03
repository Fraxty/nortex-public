//! Übersetzungen für Vorlagen mit `{}`-Platzhaltern (siehe `tr_fmt`).

pub(super) const TABLE: &[(&str, &str)] = &[
    ("\n  Hilfe: {}", "\n  Help: {}"),
    ("{} (fehlt)", "{} (missing)"),
    ("{} (geplant)", "{} (planned)"),
    ("{} Änderungen", "{} changes"),
    ("{} Fehler", "{} errors"),
    ("{} offene Dokumente", "{} open documents"),
    ("{} Quelldateien", "{} source files"),
    (
        "{} Strg+F5 startet das Programm ohne Debugger.",
        "{} Ctrl+F5 starts the program without a debugger.",
    ),
    ("{} Vorkommen ersetzt.", "{} occurrences replaced."),
    ("{} Vorkommen umbenannt", "{} occurrences renamed"),
    (
        "„{}“ ist kein gültiger Bezeichner.",
        "\"{}\" is not a valid identifier.",
    ),
    ("… {} weitere nicht angezeigt", "… {} more not shown"),
    ("Änderungen an „{}“ speichern?", "Save changes to \"{}\"?"),
    ("Angemeldet: {}", "Signed in: {}"),
    ("Automatisch gespeichert ({})", "Auto-saved ({})"),
    ("Backend: {}", "Backend: {}"),
    ("Branch: {}", "Branch: {}"),
    (
        "Breakpoint in Zeile {} entfernt",
        "Breakpoint removed at line {}",
    ),
    (
        "Breakpoint in Zeile {} gesetzt",
        "Breakpoint set at line {}",
    ),
    ("Breakpoints ({})", "Breakpoints ({})"),
    ("Build erfolgreich: {}", "Build succeeded: {}"),
    (
        "Build-Worker konnte nicht gestartet werden: {}",
        "Build worker could not be started: {}",
    ),
    (
        "Datei konnte nicht geöffnet werden: {}",
        "File could not be opened: {}",
    ),
    (
        "Diagnose-Worker konnte nicht starten: {}",
        "Diagnostics worker could not start: {}",
    ),
    (
        "Dokumentation nicht geöffnet: {}",
        "Documentation not opened: {}",
    ),
    ("Editor-Layout: {}", "Editor layout: {}"),
    ("Einstellungen exportiert: {}", "Settings exported: {}"),
    ("Einstellungen importiert: {}", "Settings imported: {}"),
    (
        "Einstellungen konnten nicht gespeichert werden: {}",
        "Settings could not be saved: {}",
    ),
    ("Einstellungsdatei: {}", "Settings file: {}"),
    ("Export fehlgeschlagen: {}", "Export failed: {}"),
    ("Geöffnet: {}", "Opened: {}"),
    (
        "Git-Abfrage konnte nicht gestartet werden: {}",
        "Git query could not be started: {}",
    ),
    (
        "Handbuch konnte nicht geöffnet werden: {}",
        "Manual could not be opened: {}",
    ),
    (
        "Keine Vorkommen von „{}“ gefunden.",
        "No occurrences of \"{}\" found.",
    ),
    ("nortex_runtime.lib: {}", "nortex_runtime.lib: {}"),
    ("nortexc.exe: {}", "nortexc.exe: {}"),
    (
        "Nur .nx-Dateien werden geöffnet: {}",
        "Only .nx files are opened: {}",
    ),
    ("Offline: {}", "Offline: {}"),
    ("Overflow-Checks: {}", "Overflow checks: {}"),
    (
        "Programm konnte nicht gestartet werden: {}",
        "Program could not be started: {}",
    ),
    ("Projekt geladen ({} Dateien)", "Project loaded ({} files)"),
    (
        "Projekt konnte nicht aktualisiert werden: {}",
        "Project could not be refreshed: {}",
    ),
    (
        "Projektaktualisierung fehlgeschlagen: {}",
        "Project refresh failed: {}",
    ),
    (
        "Projektordner existiert nicht: {}",
        "Project folder does not exist: {}",
    ),
    (
        "Projektordner konnte nicht geöffnet werden: {}",
        "Project folder could not be opened: {}",
    ),
    ("Shell nicht startbar: {}", "Shell cannot be started: {}"),
    ("Speichern fehlgeschlagen: {}", "Save failed: {}"),
    ("Speichern unter fehlgeschlagen: {}", "Save as failed: {}"),
    ("Tab verschoben ({})", "Tab moved ({})"),
    ("Umbenennen: {}", "Rename: {}"),
    ("Ze {}, Sp {}", "Ln {}, Col {}"),
];

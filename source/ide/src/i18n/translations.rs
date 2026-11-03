//! Deutsch → Englisch. Schlüssel sind die deutschen Oberflächentexte (siehe `crate::i18n`).

use std::collections::HashMap;
use std::sync::LazyLock;

mod bundle;
mod feedback;
mod git;
mod messages;
mod more;

pub(super) fn english(german: &str) -> Option<&'static str> {
    static MAP: LazyLock<HashMap<&'static str, &'static str>> =
        LazyLock::new(|| all_entries().collect());
    MAP.get(german).copied()
}

pub(super) fn all_entries() -> impl Iterator<Item = (&'static str, &'static str)> {
    TABLE
        .iter()
        .chain(more::TABLE.iter())
        .chain(feedback::TABLE.iter())
        .chain(git::TABLE.iter())
        .chain(bundle::TABLE.iter())
        .chain(messages::TABLE.iter())
        .copied()
}

const TABLE: &[(&str, &str)] = &[
    ("+ Neue Datei  Strg+N", "+ New File  Ctrl+N"),
    ("Abbrechen", "Cancel"),
    (
        "Abbrechen laufender Programme ist nicht implementiert.",
        "Cancelling running programs is not implemented.",
    ),
    ("Abmelden", "Sign out"),
    ("Aktualisieren", "Refresh"),
    ("Alle", "All"),
    ("Alle Ebenen", "All levels"),
    ("Alle Ordner einklappen", "Collapse all folders"),
    ("Alle entfernen", "Remove all"),
    ("Alle ersetzen", "Replace all"),
    (
        "Alle lokalen Funktionen laufen ohne Konto.",
        "All local features work without an account.",
    ),
    ("Alle zurücksetzen", "Reset all"),
    ("Alles auswählen  Strg+A", "Select All  Ctrl+A"),
    ("Anmelden…", "Sign in…"),
    ("Ansicht", "View"),
    (
        "Anzeige der aktuellen Belegung; Anpassen ist noch nicht möglich.",
        "Shows the current bindings; customizing is not available yet.",
    ),
    ("Aufrufliste", "Call stack"),
    ("Ausschneiden  Strg+X", "Cut  Ctrl+X"),
    ("Bearbeiten", "Edit"),
    ("Beenden", "Exit"),
    ("Befehl eingeben …", "Enter command …"),
    (
        "Befehl eingeben, Enter sendet",
        "Enter a command, Enter sends",
    ),
    ("Block-Kommentar  Strg+/", "Block Comment  Ctrl+/"),
    (
        "Breakpoints brauchen eine gespeicherte Datei.",
        "Breakpoints need a saved file.",
    ),
    ("Build & Run", "Build & Run"),
    (
        "Build läuft im Hintergrund ...",
        "Build is running in the background ...",
    ),
    ("Clear", "Clear"),
    ("Dashboard (NORTEX)", "Dashboard (NORTEX)"),
    ("Datei", "File"),
    ("Datei gespeichert", "File saved"),
    ("Datei öffnen", "Open file"),
    (
        "Debugging braucht eine gespeicherte Datei.",
        "Debugging needs a saved file.",
    ),
    (
        "Der Projektordner ist kein Git-Repository.",
        "The project folder is not a Git repository.",
    ),
    (
        "Der Sidecar ist noch nicht angebunden.",
        "The sidecar is not connected yet.",
    ),
    (
        "Diagnoseinformationen in die Zwischenablage kopiert.",
        "Diagnostic information copied to the clipboard.",
    ),
    (
        "Diagnoseinformationen kopieren",
        "Copy diagnostic information",
    ),
    ("Dichte", "Density"),
    ("Dokumentationsordner öffnen", "Open documentation folder"),
    (
        "Editor für die Programmiersprache NORTEX.",
        "Editor for the NORTEX programming language.",
    ),
    ("Editor-Layout", "Editor layout"),
    ("Editor-Zoom", "Editor zoom"),
    ("Einfügen  Strg+V", "Paste  Ctrl+V"),
    ("Einstellungen", "Settings"),
    ("Einstellungen exportieren", "Export settings"),
    ("Einstellungen importieren", "Import settings"),
    ("Einstellungen suchen", "Search settings"),
    ("Einstellungen zurückgesetzt", "Settings reset"),
    ("Entfernen", "Remove"),
    ("Ersetzen", "Replace"),
    ("Ersetzen durch...", "Replace with..."),
    (
        "Ersetzen ein-/ausblenden (Strg+H)",
        "Toggle replace (Ctrl+H)",
    ),
    ("Erste Schritte", "Getting started"),
    (
        "Erweiterungen: nicht implementiert.",
        "Extensions: not implemented.",
    ),
    ("Explorer (Strg+B)", "Explorer (Ctrl+B)"),
    ("Exportieren…", "Export…"),
    (
        "Git-Status wird im Hintergrund geladen …",
        "Loading Git status in the background …",
    ),
    ("Handbuch (PDF) öffnen", "Open manual (PDF)"),
    ("Handbuch im Fenster", "Manual in window"),
    ("Hilfe", "Help"),
    ("Im Browser anmelden", "Sign in via browser"),
    (
        "Import fehlgeschlagen: keine gültige Einstellungsdatei.",
        "Import failed: not a valid settings file.",
    ),
    ("Importieren…", "Import…"),
    ("Info", "About"),
    ("JSON", "JSON"),
    ("KI", "AI"),
    ("KI-Assistenz", "AI assistant"),
    ("Kategorie zurücksetzen", "Reset category"),
    ("Kein Git-Repository.", "No Git repository."),
    (
        "Kein KI-Anbieter konfiguriert.",
        "No AI provider configured.",
    ),
    ("Kein Split-Pane zu schließen.", "No split pane to close."),
    ("Keine Auswahl vorhanden.", "No selection."),
    (
        "Keine Datei: Tab hierher ziehen oder Datei öffnen.",
        "No file: drag a tab here or open a file.",
    ),
    ("Keine Treffer.", "No results."),
    ("Keine aktive Sitzung.", "No active session."),
    (
        "Keine. Mit F9 im Editor setzen.",
        "None. Press F9 in the editor to set one.",
    ),
    ("Kompiliere...", "Compiling..."),
    ("Konto", "Account"),
    ("Konto und Synchronisierung…", "Account and sync…"),
    ("Kopieren  Strg+C", "Copy  Ctrl+C"),
    ("Layout zurücksetzen", "Reset layout"),
    ("Liste der Erweiterungen", "List of extensions"),
    ("Lizenz", "License"),
    ("Minimap anzeigen (Strg+Alt+M)", "Show minimap (Ctrl+Alt+M)"),
    ("NORTEX Code – Handbuch", "NORTEX Code – Manual"),
    ("NORTEX-Dateien", "NORTEX files"),
    ("Neu", "New"),
    ("Neue Datei  Strg+N", "New File  Ctrl+N"),
    ("Neues Dokument", "New document"),
    (
        "Noch keine zuletzt geöffneten Dateien.",
        "No recently opened files yet.",
    ),
    (
        "Noch nicht angebunden – es wird kein Code gesendet.",
        "Not connected yet – no code is sent.",
    ),
    ("Nortex-Konto", "Nortex account"),
    ("Nortex-Konto öffnen…", "Open Nortex account…"),
    ("Nächstes (Enter)", "Next (Enter)"),
    ("Nächstes Vorkommen auswählen", "Select next occurrence"),
    ("Ordner öffnen…", "Open folder…"),
    ("Overflow-Checks", "Overflow checks"),
    (
        "Pane schließen  Strg+Umschalt+W",
        "Close Pane  Ctrl+Shift+W",
    ),
    ("Projekt aktualisieren", "Refresh project"),
    ("Projekt wird aktualisiert …", "Refreshing project …"),
    (
        "Projektordner im Explorer öffnen",
        "Show project folder in Explorer",
    ),
    ("Projektordner schließen", "Close project folder"),
    ("Rückgängig", "Undo"),
    ("Schließen", "Close"),
    ("Schließen (Esc)", "Close (Esc)"),
    ("Seitenleiste", "Side bar"),
    ("Senden", "Send"),
    ("Sidecar (Strg+Umschalt+L)", "Sidecar (Ctrl+Shift+L)"),
    ("Speichern", "Save"),
    ("Speichern  Strg+S", "Save  Ctrl+S"),
    ("Speichern unter…", "Save As…"),
    ("Standardwert", "Default value"),
    ("Startseite", "Home"),
    ("Suchen", "Search"),
    ("Suchen  Strg+F", "Find  Ctrl+F"),
    ("Suchen (Enter)", "Search (Enter)"),
    ("Suchen...", "Find..."),
    ("Suchen/Ersetzen  Strg+H", "Find/Replace  Ctrl+H"),
    (
        "Synchronisierung (wirkt erst nach Anmeldung)",
        "Sync (takes effect after sign-in)",
    ),
    ("Tastatur-Kürzel", "Keyboard shortcuts"),
    ("Tastenkürzel", "Keyboard shortcuts"),
    (
        "Terminal / Diagnosen (Strg+J)",
        "Terminal / Diagnostics (Ctrl+J)",
    ),
    ("Theme", "Theme"),
    (
        "Themes (Farbschema, Dichte, Zoom)",
        "Themes (color scheme, density, zoom)",
    ),
    ("UI-Zoom", "UI zoom"),
    ("Ungespeicherte Änderungen", "Unsaved changes"),
    (
        "Ungespeicherte Änderungen: Speichern oder Verwerfen.",
        "Unsaved changes: save or discard.",
    ),
    ("Ungültige RegEx", "Invalid regex"),
    ("Ungültige Zeilennummer.", "Invalid line number."),
    ("Update-Check", "Update check"),
    ("Variablen", "Variables"),
    ("Verwerfen", "Discard"),
    ("Vom Standard abweichend", "Modified from default"),
    ("Vorheriges (Umschalt+Enter)", "Previous (Shift+Enter)"),
    (
        "Vorschläge und Kontext erscheinen hier.",
        "Suggestions and context appear here.",
    ),
    ("Wiederholen", "Redo"),
    ("Zeile duplizieren", "Duplicate line"),
    ("Zeile nach oben", "Move line up"),
    ("Zeile nach unten", "Move line down"),
    ("Zen-Modus (F11)", "Zen mode (F11)"),
    ("Zoom zurücksetzen", "Reset zoom"),
    ("Zu Zeile springen", "Go to line"),
    ("Zu Zeile springen…", "Go to Line…"),
    (
        "Zum Bauen zuerst einen Dateipfad angeben und speichern.",
        "To build, first specify a file path and save.",
    ),
    ("geplant", "planned"),
    (
        "git ist nicht installiert oder nicht im PATH.",
        "git is not installed or not on PATH.",
    ),
    ("git ist nicht installiert.", "git is not installed."),
    ("Öffnen", "Open"),
    ("Öffnen…", "Open…"),
    ("■ Stopp", "■ Stop"),
    ("▶ Debuggen  F5", "▶ Debug  F5"),
    ("▶ Run  Ctrl+F5", "▶ Run  Ctrl+F5"),
];

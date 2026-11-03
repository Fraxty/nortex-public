//! Übersetzungen für Git-Panel und Diagnose-Schnellkorrekturen.

pub(super) const TABLE: &[(&str, &str)] = &[
    ("Beide übernehmen", "Accept both"),
    ("Diff (Index)", "Diff (index)"),
    ("Diff", "Diff"),
    ("Diff schließen", "Close diff"),
    ("Diff: {}", "Diff: {}"),
    ("Eigene übernehmen", "Accept ours"),
    ("Entstagen", "Unstage"),
    ("Fremde übernehmen", "Accept theirs"),
    ("Git-Fehler: {}", "Git error: {}"),
    ("Hunk entstagen", "Unstage hunk"),
    ("Hunk stagen", "Stage hunk"),
    (
        "Keine Änderungen in dieser Datei.",
        "No changes in this file.",
    ),
    ("Kein Quick Fix verfügbar.", "No quick fix available."),
    ("Konflikt {} (Zeilen {}–{})", "Conflict {} (lines {}–{})"),
    ("Leerzeichen am Zeilenende", "Trailing whitespace"),
    (
        "Leerzeichen am Zeilenende entfernen",
        "Remove trailing whitespace",
    ),
    ("Meinten Sie '{}'?", "Did you mean '{}'?"),
    ("Offene Aufgabe im Kommentar", "Open task in comment"),
    ("Quick Fix", "Quick Fix"),
    ("Quick Fix angewendet: {}", "Quick fix applied: {}"),
    ("Semikolon einfügen", "Insert semicolon"),
    ("Stagen", "Stage"),
];

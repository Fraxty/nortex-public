//! VS-Code-kompatible Tastenbelegung: reine Zuordnung Tastenkombination -> Aktion.

use eframe::egui::{Event, InputState, Key, Modifiers};

use crate::editor::split::{Direction, SplitMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    FocusExplorer,
    FocusSearch,
    FocusGit,
    FocusDebugger,
    FocusExtensions,
    ToggleSidebar,
    TogglePanel,
    ToggleSidecar,
    CommandPalette,
    QuickOpen,
    NextTab,
    Save,
    SaveAs,
    Undo,
    Redo,
    FindInFile,
    Replace,
    ToggleComment,
    MoveLineUp,
    MoveLineDown,
    DuplicateLine,
    SelectNextOccurrence,
    Build,
    BuildAndRun,
    StartDebugging,
    StopExecution,
    ShowProblems,
    ShowOutput,
    Split(SplitMode),
    ClosePane,
    RenameSymbol,
    ToggleMinimap,
    FocusPane(usize),
    FocusDirection(Direction),
    NewFile,
    ToggleZen,
    InsertMain,
    ToggleBreakpoint,
}

/// Anzeigetabelle der Tastenkürzel (Kombination, Beschreibung); Grundlage für Hilfe und Einstellungen.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("Strg+N", "Neue Datei"),
    ("Strg+S", "Speichern"),
    ("Strg+Umschalt+S", "Speichern unter"),
    ("Strg+Z", "Rückgängig"),
    ("Strg+Y", "Wiederholen"),
    ("Strg+F", "Suchen"),
    ("Strg+H", "Ersetzen"),
    ("Strg+/", "Zeilenkommentar umschalten"),
    ("Strg+D", "Nächstes Vorkommen auswählen"),
    ("Alt+Auf / Alt+Ab", "Zeile verschieben"),
    ("Umschalt+Alt+Ab", "Zeile duplizieren"),
    ("Strg+B", "Explorer ein/aus"),
    ("Strg+J", "Konsole ein/aus"),
    ("Strg+P", "Schnellöffnen"),
    ("Strg+Leertaste", "Autovervollständigung öffnen"),
    ("Strg+Umschalt+P / F1", "Befehlspalette"),
    ("Strg+Alt+M", "Minimap umschalten"),
    ("Strg+Alt+B", "main-Gerüst einfügen"),
    ("Strg+Umschalt+B", "Build"),
    ("Strg+F5", "Build und Ausführen"),
    ("F5", "Debugging starten (noch ohne Backend)"),
    ("F9", "Breakpoint umschalten"),
    ("F2", "Symbol umbenennen"),
    ("F11 / Esc", "Zen-Modus ein/aus"),
];

/// `Alt+Auf/Ab` verschiebt Zeilen; Pane-Fokus vertikal: `Strg+Alt+Auf/Ab`.
pub fn dispatch(modifiers: Modifiers, key: Key) -> Option<Action> {
    let (ctrl, shift, alt) = (modifiers.command, modifiers.shift, modifiers.alt);
    let arrow = |key: Key| match key {
        Key::ArrowLeft => Some(Direction::Left),
        Key::ArrowRight => Some(Direction::Right),
        Key::ArrowUp => Some(Direction::Up),
        Key::ArrowDown => Some(Direction::Down),
        _ => None,
    };
    Some(match (ctrl, shift, alt, key) {
        (true, true, false, Key::E) => Action::FocusExplorer,
        (true, true, false, Key::F) => Action::FocusSearch,
        (true, true, false, Key::G) => Action::FocusGit,
        (true, true, false, Key::D) => Action::FocusDebugger,
        (true, true, false, Key::X) => Action::FocusExtensions,
        (true, true, false, Key::P) => Action::CommandPalette,
        (true, true, false, Key::S) => Action::SaveAs,
        (true, true, false, Key::B) => Action::Build,
        (true, true, false, Key::M) => Action::ShowProblems,
        (true, true, false, Key::U) => Action::ShowOutput,
        (true, true, false, Key::L) => Action::ToggleSidecar,
        (true, true, false, Key::W) => Action::ClosePane,
        (true, true, false, Key::Z) => Action::Redo,
        (true, true, false, Key::Backslash) => Action::Split(SplitMode::TwoRows),
        (true, true, false, Key::Num2) => Action::Split(SplitMode::TwoColumns),
        (true, true, false, Key::Num3) => Action::Split(SplitMode::TwoRows),
        (true, true, false, Key::Num4) => Action::Split(SplitMode::FourGrid),
        (true, false, false, Key::B) => Action::ToggleSidebar,
        (true, false, false, Key::J) => Action::TogglePanel,
        (true, false, false, Key::P) => Action::QuickOpen,
        (true, false, false, Key::Tab) => Action::NextTab,
        (true, false, false, Key::S) => Action::Save,
        (true, false, false, Key::Z) => Action::Undo,
        (true, false, false, Key::Y) => Action::Redo,
        (true, false, false, Key::F) => Action::FindInFile,
        (true, false, false, Key::H) => Action::Replace,
        (true, false, false, Key::Slash) => Action::ToggleComment,
        (true, false, false, Key::D) => Action::SelectNextOccurrence,
        (true, false, false, Key::Backslash) => Action::Split(SplitMode::TwoColumns),
        (true, false, false, Key::F5) => Action::BuildAndRun,
        (true, false, false, Key::Num1) => Action::FocusPane(0),
        (true, false, false, Key::Num2) => Action::FocusPane(1),
        (true, false, false, Key::Num3) => Action::FocusPane(2),
        (true, false, false, Key::Num4) => Action::FocusPane(3),
        (true, false, true, Key::M) => Action::ToggleMinimap,
        (true, false, true, Key::B) => Action::InsertMain,
        (true, false, false, Key::N) => Action::NewFile,
        (false, false, false, Key::F11) => Action::ToggleZen,
        (true, false, true, key) => Action::FocusDirection(arrow(key)?),
        (false, false, false, Key::F1) => Action::CommandPalette,
        (false, false, false, Key::F2) => Action::RenameSymbol,
        // Strg+Umschalt+M bleibt „Probleme“ (VS-Code-Belegung); Minimap: Strg+Alt+M.
        (false, false, false, Key::F5) => Action::StartDebugging,
        (false, false, false, Key::F9) => Action::ToggleBreakpoint,
        (false, true, false, Key::F5) => Action::StopExecution,
        (false, false, true, Key::ArrowUp) => Action::MoveLineUp,
        (false, false, true, Key::ArrowDown) => Action::MoveLineDown,
        (false, false, true, Key::ArrowLeft) => Action::FocusDirection(Direction::Left),
        (false, false, true, Key::ArrowRight) => Action::FocusDirection(Direction::Right),
        (false, true, true, Key::ArrowDown) => Action::DuplicateLine,
        _ => return None,
    })
}

/// Sammelt die ausgelösten Aktionen dieses Frames und verbraucht die zugehörigen Tasten.
pub fn poll(input: &mut InputState) -> Vec<Action> {
    let hits: Vec<(Modifiers, Key, Action)> = input
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => dispatch(*modifiers, *key).map(|action| (*modifiers, *key, action)),
            _ => None,
        })
        .collect();
    for (modifiers, key, _) in &hits {
        input.consume_key(*modifiers, *key);
    }
    hits.into_iter().map(|(_, _, action)| action).collect()
}

#[cfg(test)]
mod shortcut_table_tests {
    use super::SHORTCUTS;
    use std::collections::HashSet;

    #[test]
    fn shortcut_table_has_no_duplicate_combinations() {
        let mut seen = HashSet::new();
        for (keys, _) in SHORTCUTS {
            assert!(seen.insert(*keys), "doppelte Kombination: {keys}");
        }
    }
}

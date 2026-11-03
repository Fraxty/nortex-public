use nortex_ide::editor::refactor::{self, RenameState};
use nortex_ide::editor::scope::{self, ScopeKind};
use nortex_ide::ui::keymap::{Action, dispatch};
use nortex_ide::ui::minimap;
use nortex_ide::ui::settings::Settings;

use eframe::egui::{Key, Modifiers};

const SOURCE: &str = "// summe im Kommentar\n\
struct Vector3 {\n\
    int x;\n\
}\n\
\n\
public int main() {\n\
    int summe = 2 + 3;\n\
    string s = \"summe\";\n\
    return summe;\n\
}\n";

#[test]
fn rename_replaces_identifier_tokens_only() {
    let (renamed, count) = refactor::rename_identifier(SOURCE, "summe", "total").unwrap();
    assert_eq!(count, 2);
    assert!(renamed.contains("int total = 2 + 3;"));
    assert!(renamed.contains("return total;"));
    assert!(renamed.contains("// summe im Kommentar"));
    assert!(renamed.contains("\"summe\""));
}

#[test]
fn rename_rejects_keywords_and_invalid_names() {
    assert!(refactor::rename_identifier(SOURCE, "summe", "return").is_err());
    assert!(refactor::rename_identifier(SOURCE, "summe", "1abc").is_err());
    assert!(refactor::rename_identifier(SOURCE, "summe", "").is_err());
    assert!(refactor::rename_identifier(SOURCE, "gibtesnicht", "x").is_err());
}

#[test]
fn identifier_is_found_under_and_directly_after_the_cursor() {
    let text = "int summe = 1;";
    assert_eq!(refactor::identifier_at(text, 6).as_deref(), Some("summe"));
    assert_eq!(refactor::identifier_at(text, 9).as_deref(), Some("summe"));
    assert_eq!(refactor::identifier_at(text, 1), None);
}

#[test]
fn rename_is_unicode_safe() {
    let text = "// Größe äö\nint breite = 1;\nstring s = \"ü breite\";\nreturn breite;\n";
    let (renamed, count) = refactor::rename_identifier(text, "breite", "width").unwrap();
    assert_eq!(count, 2);
    assert!(renamed.starts_with("// Größe äö\n"));
    assert!(renamed.contains("\"ü breite\""));
    assert!(renamed.ends_with("return width;\n"));
}

#[test]
fn rename_state_opens_with_full_name_and_closes() {
    let mut state = RenameState::default();
    state.open("summe".into(), (10.0, 20.0));
    assert!(state.is_active && state.fresh);
    assert_eq!(state.new_name, "summe");
    state.close();
    assert!(!state.is_active);
}

#[test]
fn breadcrumb_scope_path_follows_the_cursor_line() {
    let path = |line| {
        scope::scope_path(SOURCE, line)
            .iter()
            .map(|scope| scope.label())
            .collect::<Vec<_>>()
    };
    assert!(path(0).is_empty());
    assert_eq!(path(2), ["struct Vector3"]);
    assert_eq!(path(6), ["fn main()"]);
    assert_eq!(path(4), Vec::<String>::new());
}

#[test]
fn nested_methods_resolve_outer_to_inner() {
    let text = "class Box {\n  public int area() {\n    return 1;\n  }\n}\n";
    let path = scope::scope_path(text, 2);
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].kind, ScopeKind::Class);
    assert_eq!(path[1].kind, ScopeKind::Function);
    assert_eq!(path[1].label(), "fn area()");
}

#[test]
fn calls_and_control_flow_are_not_scopes() {
    let text = "public int main() {\n  if (1 < 2) { return foo(1); }\n  return 0;\n}\n";
    let labels: Vec<_> = scope::scopes(text).iter().map(|s| s.label()).collect();
    assert_eq!(labels, ["fn main()"]);
}

#[test]
fn unterminated_scope_extends_to_end_of_file() {
    let path = scope::scope_path("public int main() {\n  return 0;\n", 1);
    assert_eq!(path.len(), 1);
}

#[test]
fn line_index_helpers_round_trip() {
    let text = "ab\ncd\nef";
    assert_eq!(scope::line_of_char(text, 4), 1);
    assert_eq!(scope::char_index_of_line(text, 2), 6);
    assert_eq!(scope::char_index_of_line(text, 0), 0);
}

#[test]
fn f2_and_minimap_shortcuts_are_bound() {
    assert_eq!(
        dispatch(Modifiers::NONE, Key::F2),
        Some(Action::RenameSymbol)
    );
    let ctrl_alt = Modifiers {
        command: true,
        ctrl: true,
        alt: true,
        ..Modifiers::NONE
    };
    assert_eq!(dispatch(ctrl_alt, Key::M), Some(Action::ToggleMinimap));
    // Probleme-Ansicht behält Strg+Umschalt+M.
    let ctrl_shift = Modifiers {
        command: true,
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    assert_eq!(dispatch(ctrl_shift, Key::M), Some(Action::ShowProblems));
}

#[test]
fn minimap_geometry_maps_clicks_and_viewport() {
    assert_eq!(minimap::px_per_line(10, 500.0), 2.0);
    assert!((minimap::px_per_line(1000, 500.0) - 0.5).abs() < 1e-6);
    let (top, height) = minimap::viewport_span(0.0, 100.0, 1000.0, 200.0);
    assert_eq!(top, 0.0);
    assert!((height - 20.0).abs() < 1e-3);
    // Klick in die Mitte der Miniatur zentriert die Mitte des Dokuments.
    let offset = minimap::scroll_for_click(100.0, 200.0, 1000.0, 100.0);
    assert!((offset - 450.0).abs() < 1e-3);
    assert_eq!(minimap::scroll_for_click(0.0, 200.0, 1000.0, 100.0), 0.0);
    assert_eq!(
        minimap::scroll_for_click(200.0, 200.0, 1000.0, 100.0),
        900.0
    );
}

#[test]
fn minimap_setting_defaults_off_and_round_trips_under_dotted_key() {
    let mut settings = Settings::default();
    assert!(!settings.minimap_enabled);
    settings.minimap_enabled = true;
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("\"editor.minimap.enabled\":true"), "{json}");
    let loaded = Settings::from_json_lenient(&json).unwrap();
    assert!(loaded.minimap_enabled);
    let wrong_type = Settings::from_json_lenient("{\"editor.minimap.enabled\": 3}").unwrap();
    assert!(!wrong_type.minimap_enabled);
}

use nortex_ide::editor::completion::{Completion, MAX_SUGGESTIONS, prefix_at, suggest};
use nortex_ide::editor::diagnostics::{
    DiagnosticSeverity, MAX_INLINE_DIAGNOSTICS, char_index, normalize, parse_compiler_log,
    squiggle_points,
};

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

#[test]
fn prefix_needs_two_identifier_chars() {
    assert_eq!(prefix_at(&chars("wh"), 2).as_deref(), Some("wh"));
    assert_eq!(prefix_at(&chars("w"), 1), None);
    assert_eq!(prefix_at(&chars("1a"), 2), None);
    assert_eq!(prefix_at(&chars("x = wh"), 6).as_deref(), Some("wh"));
    assert_eq!(prefix_at(&chars("wh "), 3), None);
}

#[test]
fn suggestions_are_capped_deduplicated_and_exclude_exact_match() {
    let text = "int counter; int count2; int counted; int country;";
    let items = suggest("co", &chars(text), 0);
    assert!(items.len() <= MAX_SUGGESTIONS);
    assert!(items.contains(&"const".to_string()));
    let exact = suggest("int", &chars(text), 0);
    assert!(!exact.contains(&"int".to_string()));
    assert!(exact.contains(&"interface".to_string()));
}

#[test]
fn document_identifiers_are_offered_but_not_the_word_being_typed() {
    let text = "int total_sum = 1;\ntot";
    let cursor = text.chars().count();
    let items = suggest("tot", &chars(text), cursor);
    assert_eq!(items, vec!["total_sum".to_string()]);
}

#[test]
fn selection_wraps_in_both_directions_and_accept_replaces_prefix() {
    let text = "pu";
    let mut popup = Completion::open(text, 2).unwrap();
    assert_eq!(popup.items[0], "public");
    popup.step(-1);
    assert_eq!(popup.selected, popup.items.len() - 1);
    popup.step(1);
    assert_eq!(popup.selected, 0);
    let (new_text, cursor) = popup.accept(text).unwrap();
    assert_eq!((new_text.as_str(), cursor), ("public", 6));
}

#[test]
fn accept_keeps_text_after_cursor_and_handles_unicode() {
    let text = "ä = whi + 1";
    let popup = Completion::open(text, 7).unwrap();
    let (new_text, cursor) = popup.accept(text).unwrap();
    assert_eq!(new_text, "ä = while + 1");
    assert_eq!(cursor, 9);
}

#[test]
fn no_popup_without_matches() {
    assert!(Completion::open("zzq", 3).is_none());
}

#[test]
fn parses_compiler_format_and_filters_by_file() {
    let log = "error[NX1001]: Ungültiger Wert\n --> src/main.nx:2:9\n  |\nwarning: unbenutzt\n --> other.nx:1:1\n";
    let all = parse_compiler_log(log, None);
    assert_eq!(all.len(), 2);
    assert_eq!(
        (all[0].line, all[0].col_start, all[0].severity),
        (2, 9, DiagnosticSeverity::Error)
    );
    let only = parse_compiler_log(log, Some(r"C:\proj\src\main.nx"));
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].message, "Ungültiger Wert");
}

#[test]
fn parses_inline_span_form() {
    let items = parse_compiler_log(
        "error at line 3, col 5..9: kaputt\nwarning at line 1, col 2: hm",
        None,
    );
    assert_eq!(items.len(), 2);
    assert_eq!(
        (items[0].line, items[0].col_start, items[0].col_end),
        (3, 5, 9)
    );
    assert_eq!(items[1].severity, DiagnosticSeverity::Warning);
}

#[test]
fn garbage_and_empty_logs_yield_nothing() {
    assert!(parse_compiler_log("", None).is_empty());
    assert!(
        parse_compiler_log("--> x:1:1\nerror at line x, col y: z\nerror[: --> \n", None).is_empty()
    );
}

#[test]
fn count_is_limited() {
    let log: String = (0..1000)
        .map(|i| format!("error at line {}, col 1: m\n", i + 1))
        .collect();
    assert_eq!(parse_compiler_log(&log, None).len(), MAX_INLINE_DIAGNOSTICS);
}

#[test]
fn normalize_drops_out_of_range_and_widens_empty_spans() {
    let source = "let value = 1;\nx";
    let mut items = parse_compiler_log(
        "error at line 1, col 5: a\nerror at line 9, col 1: b\nerror at line 1, col 99: c\nerror at line 0, col 1: d",
        None,
    );
    normalize(&mut items, source);
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].col_start, items[0].col_end), (5, 10));
}

#[test]
fn char_index_is_unicode_and_bounds_safe() {
    let source = "äö\nxyz";
    assert_eq!(char_index(source, 1, 2), Some(1));
    assert_eq!(char_index(source, 2, 1), Some(3));
    assert_eq!(char_index(source, 3, 1), None);
    assert_eq!(char_index(source, 1, 99), None);
    assert_eq!(char_index(source, 0, 1), None);
}

#[test]
fn squiggle_geometry_matches_amplitude_and_period() {
    let points = squiggle_points(10.0, 22.0, 50.0);
    assert_eq!(points.first(), Some(&(10.0, 50.0)));
    assert_eq!(points.last().map(|p| p.0), Some(22.0));
    assert!(points.iter().all(|p| p.1 == 50.0 || p.1 == 48.0));
    assert!(
        points
            .windows(2)
            .all(|w| w[1].0 - w[0].0 <= 2.0 + f32::EPSILON)
    );
    assert!(squiggle_points(5.0, 5.0, 0.0).is_empty());
    assert!(squiggle_points(f32::NAN, 5.0, 0.0).is_empty());
    assert!(squiggle_points(0.0, 1.0e9, 0.0).len() <= 4_097);
}

use nortex_ide::editor::find_replace::{
    FindError, FindOptions, FindState, find_all, nearest, replace_all, replace_match,
};

const PLAIN: FindOptions = FindOptions {
    case_sensitive: false,
    whole_word: false,
    regex: false,
};
const REGEX: FindOptions = FindOptions {
    case_sensitive: true,
    whole_word: false,
    regex: true,
};

#[test]
fn plain_search_is_case_insensitive_by_default() {
    let found = find_all("Foo foo FOO", "foo", PLAIN).unwrap();
    assert_eq!(found.len(), 3);
    let exact = FindOptions {
        case_sensitive: true,
        ..PLAIN
    };
    assert_eq!(find_all("Foo foo FOO", "foo", exact).unwrap().len(), 1);
}

#[test]
fn plain_search_escapes_regex_metacharacters() {
    assert_eq!(find_all("a.b axb", "a.b", PLAIN).unwrap().len(), 1);
}

#[test]
fn whole_word_ignores_substrings() {
    let words = FindOptions {
        whole_word: true,
        ..PLAIN
    };
    assert_eq!(find_all("fn fnord fn", "fn", words).unwrap().len(), 2);
}

#[test]
fn char_offsets_handle_multibyte_text() {
    let found = find_all("äö x äö", "x", PLAIN).unwrap();
    assert_eq!((found[0].char_start, found[0].char_end), (3, 4));
    assert_eq!((found[0].start, found[0].end), (5, 6));
}

#[test]
fn navigation_wraps_in_both_directions() {
    let mut state = FindState {
        query: "a".into(),
        ..FindState::default()
    };
    state.refresh("a a a");
    state.select_from(0);
    assert_eq!(state.current, Some(0));
    assert_eq!(state.step(true, 0).unwrap().char_start, 2);
    assert_eq!(state.step(true, 0).unwrap().char_start, 4);
    assert_eq!(state.step(true, 0).unwrap().char_start, 0);
    assert_eq!(state.step(false, 0).unwrap().char_start, 4);
    assert_eq!(state.counter(), "3/3");
    let matches = find_all("a a a", "a", PLAIN).unwrap();
    assert_eq!(nearest(&matches, 3, true), Some(2));
    assert_eq!(nearest(&matches, 3, false), Some(1));
}

#[test]
fn replace_one_and_all() {
    let text = "let x = 1; let y = 2;";
    let first = find_all(text, "let", PLAIN).unwrap()[0];
    let (once, length) = replace_match(text, &first, "let", "var", PLAIN).unwrap();
    assert_eq!(once, "var x = 1; let y = 2;");
    assert_eq!(length, 3);
    let (all, count) = replace_all(text, "let", "var", PLAIN).unwrap();
    assert_eq!((all.as_str(), count), ("var x = 1; var y = 2;", 2));
}

#[test]
fn plain_replacement_keeps_dollar_signs_literal() {
    let (out, _) = replace_all("a", "a", "$1", PLAIN).unwrap();
    assert_eq!(out, "$1");
}

#[test]
fn regex_pattern_matches() {
    let found = find_all("fn  main() { fn helper() }", r"fn\s+\w+", REGEX).unwrap();
    assert_eq!(found.len(), 2);
}

#[test]
fn capture_groups_expand_in_replace_all_and_single() {
    let text = "alpha = 10\nbeta=20";
    let (out, count) = replace_all(text, r"(\w+)\s*=\s*(\d+)", "$1: $2", REGEX).unwrap();
    assert_eq!((out.as_str(), count), ("alpha: 10\nbeta: 20", 2));
    let (named, _) = replace_all("fn a", r"fn (?<n>\w+)", "function ${n}", REGEX).unwrap();
    assert_eq!(named, "function a");
    let second = find_all(text, r"(\w+)\s*=\s*(\d+)", REGEX).unwrap()[1];
    let (single, _) = replace_match(text, &second, r"(\w+)\s*=\s*(\d+)", "$2<-$1", REGEX).unwrap();
    assert_eq!(single, "alpha = 10\n20<-beta");
}

#[test]
fn invalid_regex_is_an_error_not_a_panic() {
    let error = find_all("abc", "[a-z", REGEX).unwrap_err();
    assert!(matches!(error, FindError::InvalidRegex(_)));
    assert_eq!(error.to_string(), "Ungültige RegEx");
    assert!(replace_all("abc", "(", "x", REGEX).is_err());
    let mut state = FindState {
        query: "[a-z".into(),
        options: REGEX,
        ..FindState::default()
    };
    state.refresh("abc");
    assert!(state.matches.is_empty());
    assert_eq!(state.counter(), "Ungültige RegEx");
}

#[test]
fn empty_query_and_empty_matches_are_ignored() {
    assert!(find_all("abc", "", PLAIN).unwrap().is_empty());
    assert!(find_all("abc", "x*", REGEX).unwrap().is_empty());
    let (out, count) = replace_all("abc", "x*", "-", REGEX).unwrap();
    assert_eq!((out.as_str(), count), ("abc", 0));
}

#[test]
fn opening_takes_single_line_selection_as_query() {
    let mut state = FindState::default();
    state.open_find(Some("needle"));
    assert!(state.open && state.focus_query && !state.show_replace);
    assert_eq!(state.query, "needle");
    state.open_replace(Some("a\nb"));
    assert!(state.show_replace);
    assert_eq!(state.query, "needle");
}

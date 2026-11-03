use nortex_ide::ui::status_bar::{line_column, line_ending, position_label};

#[test]
fn position_is_one_based_and_counts_characters() {
    assert_eq!(line_column("", 0), (1, 1));
    assert_eq!(line_column("ab\ncd", 4), (2, 2));
    assert_eq!(line_column("é😊x", 2), (1, 3));
    assert_eq!(position_label("a\nbc", 3), "Ze 2, Sp 2");
}

#[test]
fn position_clamps_past_the_end() {
    assert_eq!(line_column("a\nb", 99), (2, 2));
}

#[test]
fn line_endings_are_detected() {
    assert_eq!(line_ending("a\nb\n"), "LF");
    assert_eq!(line_ending("a\r\nb\r\n"), "CRLF");
    assert_eq!(line_ending("a\r\nb\n"), "Gemischt");
    assert_eq!(line_ending("ohne umbruch"), "LF");
}

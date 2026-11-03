//! Line-number gutter text. Pure helpers so the UI stays thin and testable.

use std::collections::BTreeSet;

/// Number of visual rows the editor shows for `text` (a trailing newline adds an empty row).
pub fn line_count(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'\n').count() + 1
}

/// Right-aligned numbers `1..=count`, one per row, padded to at least two digits.
pub fn gutter_text(count: usize) -> String {
    let width = count.to_string().len().max(2);
    let mut text = String::with_capacity(count * (width + 1));
    for number in 1..=count {
        if number > 1 {
            text.push('\n');
        }
        text.push_str(&format!("{number:>width$}"));
    }
    text
}

/// Wie [`gutter_text`], mit einer zusätzlichen Spalte links, in der `●` einen Breakpoint markiert.
pub fn gutter_text_marked(count: usize, breakpoints: &BTreeSet<usize>) -> String {
    let width = count.to_string().len().max(2);
    let mut text = String::with_capacity(count * (width + 2));
    for number in 1..=count {
        if number > 1 {
            text.push('\n');
        }
        text.push(if breakpoints.contains(&number) {
            '●'
        } else {
            ' '
        });
        text.push_str(&format!("{number:>width$}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_column_shows_breakpoints_only() {
        let marked = gutter_text_marked(3, &BTreeSet::from([2]));
        assert_eq!(marked, "  1\n● 2\n  3");
    }

    #[test]
    fn counts_rows_including_trailing_empty_row() {
        assert_eq!(line_count(""), 1);
        assert_eq!(line_count("a"), 1);
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count("a\n"), 2);
    }

    #[test]
    fn gutter_is_right_aligned_and_matches_row_count() {
        assert_eq!(gutter_text(3), " 1\n 2\n 3");
        let text = gutter_text(100);
        assert_eq!(text.lines().count(), 100);
        assert!(text.starts_with("  1\n"));
    }
}

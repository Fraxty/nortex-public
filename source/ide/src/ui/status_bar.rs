//! Reine Hilfsfunktionen für die Statusleiste (ohne egui, daher headless testbar).

use crate::i18n::tr_fmt;

/// Zielplattform des Compilers, wie in der Statusleiste angezeigt.
pub const TARGET_LABEL: &str = "x64 MSVC";

/// 1-basierte Zeile und Spalte für einen Zeichenindex (egui adressiert Zeichen, keine Bytes).
pub fn line_column(text: &str, char_index: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for ch in text.chars().take(char_index) {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// Zeilenende-Stil des Textes: `CRLF`, `LF` oder `Gemischt` (ohne Umbruch: `LF`).
pub fn line_ending(text: &str) -> &'static str {
    let total = text.matches('\n').count();
    let crlf = text.matches("\r\n").count();
    match (total, crlf) {
        (_, 0) => "LF",
        (t, c) if t == c => "CRLF",
        _ => "Gemischt",
    }
}

/// Text der Positionsanzeige, z. B. `Ze 14, Sp 32`.
pub fn position_label(text: &str, char_index: usize) -> String {
    let (line, column) = line_column(text, char_index);
    tr_fmt("Ze {}, Sp {}", &[line.to_string(), column.to_string()])
}

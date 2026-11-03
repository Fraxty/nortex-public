//! Compiler diagnostics surfaced to NORTEX Code.

use std::path::Path;

use crate::i18n::{tr, tr_fmt};
use nortex_compiler::diagnostics::Severity;
use nortex_compiler::diagnostics::locale::localize_current;
use nortex_compiler::driver::{check_file_with_source, check_source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeDiagnostic {
    pub file: String,
    pub code: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub help: Option<String>,
    pub is_error: bool,
}

/// Rendert einen Diagnoseausschnitt mit Zeile und Caret-Markierung.
/// Zeilen und Spalten im Compiler sind einsbasiert; die IDE übernimmt diese Konvention.
pub fn render_excerpt(source: &str, diagnostic: &IdeDiagnostic) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let Some(line_text) = diagnostic
        .line
        .checked_sub(1)
        .and_then(|index| lines.get(index))
        .copied()
    else {
        return format!("{}: {}", diagnostic.code, diagnostic.message);
    };
    let column = diagnostic.column.saturating_sub(1);
    let marker = format!("{}^", " ".repeat(column.min(line_text.chars().count())));
    format!(
        "error[{}]: {} --> {}:{}:{}\n{}\n{}",
        diagnostic.code,
        diagnostic.message,
        diagnostic.file,
        diagnostic.line,
        diagnostic.column,
        line_text,
        marker
    )
}

/// Runs the same frontend used by `nortexc` and maps its diagnostics for the IDE.
///
/// Gehört `file` zu einer existierenden Datei, werden deren `using`-Imports wie beim
/// Build über den Modul-Resolver aufgelöst (der Editorpuffer ersetzt dabei den
/// Dateiinhalt); sonst wird `source` als Einzeldatei geprüft. Semantische Diagnosen
/// zu importierten Deklarationen trägt der Compiler weiterhin unter der Einstiegsdatei.
pub fn analyze_source(file: impl Into<String>, source: &str) -> Vec<IdeDiagnostic> {
    let file = file.into();
    let path = Path::new(&file);
    let checked = if path.is_file() {
        check_file_with_source(path, Some(source)).unwrap_or_else(|_| check_source(source))
    } else {
        check_source(source)
    };
    checked
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            // Read Copy fields before moving Strings out of `diagnostic`.
            let line = diagnostic.line();
            let column = diagnostic.column();
            let is_error = diagnostic.severity == Severity::Error;
            IdeDiagnostic {
                file: diagnostic.file.unwrap_or_else(|| file.clone()),
                code: diagnostic.code,
                message: localize_current(&diagnostic.message),
                line,
                column,
                help: diagnostic.help.map(|help| localize_current(&help)),
                is_error,
            }
        })
        .collect()
}

/// Ein Diagnoseanbieter: Name und Analysefunktion `(Dateiname, Quelltext)`.
#[derive(Clone, Copy)]
pub struct Provider {
    pub name: &'static str,
    pub analyze: fn(&str, &str) -> Vec<IdeDiagnostic>,
}

pub const MAX_DIAGNOSTICS: usize = 500;
pub const CODE_TRAILING_WHITESPACE: &str = "STYLE001";
pub const CODE_TODO: &str = "STYLE002";

/// Registrierte Anbieter in Ausführungsreihenfolge.
pub const PROVIDERS: &[Provider] = &[
    Provider {
        name: "compiler",
        analyze: |file, source| analyze_source(file, source),
    },
    Provider {
        name: "style",
        analyze: style_diagnostics,
    },
];

/// Warnungen für Leerzeichen am Zeilenende und TODO/FIXME-Kommentare.
pub fn style_diagnostics(file: &str, source: &str) -> Vec<IdeDiagnostic> {
    let mut found = Vec::new();
    for (index, raw) in source.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_end_matches([' ', '\t']);
        let make = |code: &str, column: usize, message: &str| IdeDiagnostic {
            file: file.to_string(),
            code: code.to_string(),
            message: message.to_string(),
            line: index + 1,
            column,
            help: None,
            is_error: false,
        };
        if trimmed.len() != line.len() {
            found.push(make(
                CODE_TRAILING_WHITESPACE,
                trimmed.chars().count() + 1,
                tr("Leerzeichen am Zeilenende"),
            ));
        }
        if let Some(comment) = line.find("//") {
            let text = &line[comment..];
            if let Some(offset) = ["TODO", "FIXME"].iter().filter_map(|k| text.find(k)).min() {
                let column = line[..comment + offset].chars().count() + 1;
                found.push(make(CODE_TODO, column, tr("Offene Aufgabe im Kommentar")));
            }
        }
    }
    found
}

/// Führt alle Anbieter aus; Ergebnis dedupliziert, nach Position sortiert und begrenzt
/// (Fehler haben beim Kappen Vorrang).
pub fn collect(file: &str, source: &str) -> Vec<IdeDiagnostic> {
    let mut all: Vec<IdeDiagnostic> = Vec::new();
    for provider in PROVIDERS {
        for item in (provider.analyze)(file, source) {
            if !all.contains(&item) {
                all.push(item);
            }
        }
    }
    all.sort_by_key(|item| !item.is_error);
    all.truncate(MAX_DIAGNOSTICS);
    all.sort_by(|a, b| {
        (&a.file, a.line, a.column, &a.code).cmp(&(&b.file, b.line, b.column, &b.code))
    });
    all
}

/// Textänderung; Zeilen und Spalten einsbasiert (Zeichenspalten), Ende exklusiv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickFix {
    pub title: String,
    pub edits: Vec<TextEdit>,
}

fn byte_offset(source: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let mut start = 0;
    for (index, raw) in source.split('\n').enumerate() {
        if index + 1 == line {
            let text = raw.strip_suffix('\r').unwrap_or(raw);
            let chars = text.chars().count();
            if column - 1 > chars {
                return None;
            }
            let inner = text
                .char_indices()
                .nth(column - 1)
                .map_or(text.len(), |(byte, _)| byte);
            return Some(start + inner);
        }
        start += raw.len() + 1;
    }
    None
}

/// Wendet Edits von hinten nach vorn an. `None` bei ungültigen Grenzen oder Überlappung.
pub fn apply_edits(source: &str, edits: &[TextEdit]) -> Option<String> {
    let mut ranges = Vec::with_capacity(edits.len());
    for edit in edits {
        let start = byte_offset(source, edit.start_line, edit.start_col)?;
        let end = byte_offset(source, edit.end_line, edit.end_col)?;
        if end < start {
            return None;
        }
        ranges.push((start, end, edit.new_text.as_str()));
    }
    ranges.sort_by_key(|range| (range.0, range.1));
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return None;
    }
    let mut result = source.to_string();
    for (start, end, text) in ranges.into_iter().rev() {
        result.replace_range(start..end, text);
    }
    Some(result)
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Wort an der einsbasierten Position als (Startspalte, Endspalte exklusiv, Text).
fn word_at(source: &str, line: usize, column: usize) -> Option<(usize, usize, String)> {
    let chars: Vec<char> = source
        .split('\n')
        .nth(line.checked_sub(1)?)?
        .trim_end_matches('\r')
        .chars()
        .collect();
    let mut start = column.checked_sub(1)?;
    if !chars.get(start).copied().is_some_and(is_ident) {
        return None;
    }
    while start > 0 && is_ident(chars[start - 1]) {
        start -= 1;
    }
    let mut end = start;
    while end < chars.len() && is_ident(chars[end]) {
        end += 1;
    }
    Some((start + 1, end + 1, chars[start..end].iter().collect()))
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != *cb))
                .min(row[j] + 1)
                .min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

fn similar_identifiers(source: &str, name: &str) -> Vec<String> {
    let limit = (name.chars().count() / 3).clamp(1, 2);
    let mut seen = std::collections::BTreeSet::new();
    let mut scored: Vec<(usize, String)> = Vec::new();
    for token in source.split(|c: char| !is_ident(c)) {
        let valid = !token.is_empty() && !token.starts_with(|c: char| c.is_ascii_digit());
        if valid && token != name && seen.insert(token) {
            let distance = levenshtein(name, token);
            if distance <= limit {
                scored.push((distance, token.to_string()));
            }
        }
    }
    scored.sort();
    scored.into_iter().take(3).map(|(_, token)| token).collect()
}

/// Echte, automatisch anwendbare Korrekturen für eine Diagnose.
pub fn quick_fixes(source: &str, diagnostic: &IdeDiagnostic) -> Vec<QuickFix> {
    let mut fixes = Vec::new();
    let (line, column) = (diagnostic.line, diagnostic.column);
    if diagnostic.code == CODE_TRAILING_WHITESPACE {
        let length = source
            .split('\n')
            .nth(line.wrapping_sub(1))
            .map_or(0, |text| text.trim_end_matches('\r').chars().count());
        if length >= column && column > 0 {
            fixes.push(QuickFix {
                title: tr("Leerzeichen am Zeilenende entfernen").into(),
                edits: vec![TextEdit {
                    start_line: line,
                    start_col: column,
                    end_line: line,
                    end_col: length + 1,
                    new_text: String::new(),
                }],
            });
        }
    } else if diagnostic.code == "NX1004" {
        if let Some((start, end, name)) = word_at(source, line, column) {
            for candidate in similar_identifiers(source, &name) {
                fixes.push(QuickFix {
                    title: tr_fmt("Meinten Sie '{}'?", std::slice::from_ref(&candidate)),
                    edits: vec![TextEdit {
                        start_line: line,
                        start_col: start,
                        end_line: line,
                        end_col: end,
                        new_text: candidate,
                    }],
                });
            }
        }
    } else if diagnostic.message.contains("';'")
        && !diagnostic.message.to_lowercase().contains("array")
    {
        if let Some(edit) = semicolon_edit(source, line, column) {
            fixes.push(QuickFix {
                title: tr("Semikolon einfügen").into(),
                edits: vec![edit],
            });
        }
    }
    fixes
}

/// Fügt `;` hinter dem letzten Nicht-Leerzeichen vor der gemeldeten Position ein.
fn semicolon_edit(source: &str, line: usize, column: usize) -> Option<TextEdit> {
    let lines: Vec<Vec<char>> = source
        .split('\n')
        .map(|text| text.trim_end_matches('\r').chars().collect())
        .collect();
    let mut row = line.checked_sub(1)?;
    let mut end = column.checked_sub(1)?.min(lines.get(row)?.len());
    loop {
        while end > 0 && lines[row][end - 1].is_whitespace() {
            end -= 1;
        }
        if end > 0 {
            break;
        }
        row = row.checked_sub(1)?;
        end = lines[row].len();
    }
    if lines[row][end - 1] == ';' {
        return None;
    }
    Some(TextEdit {
        start_line: row + 1,
        start_col: end + 1,
        end_line: row + 1,
        end_col: end + 1,
        new_text: ";".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_edits_is_utf8_safe_and_validates() {
        let text = "ä😊b\nüx";
        let edit = |l, c, el, ec, t: &str| TextEdit {
            start_line: l,
            start_col: c,
            end_line: el,
            end_col: ec,
            new_text: t.into(),
        };
        let out = apply_edits(text, &[edit(1, 2, 1, 3, "-"), edit(2, 1, 2, 2, "u")]).unwrap();
        assert_eq!(out, "ä-b\nux");
        assert_eq!(apply_edits(text, &[edit(1, 1, 1, 9, "")]), None);
        assert_eq!(apply_edits(text, &[edit(3, 1, 3, 1, "")]), None);
        assert_eq!(
            apply_edits(text, &[edit(1, 1, 1, 3, ""), edit(1, 2, 1, 4, "")]),
            None
        );
        assert_eq!(
            apply_edits(text, &[edit(1, 4, 2, 1, " ")]).unwrap(),
            "ä😊b üx"
        );
    }

    #[test]
    fn trailing_whitespace_is_reported_and_fixed() {
        let text = "int a;  \r\nint b;\n";
        let found = collect("a.nx", text);
        let item = found
            .iter()
            .find(|d| d.code == CODE_TRAILING_WHITESPACE)
            .unwrap();
        assert_eq!((item.line, item.column), (1, 7));
        let fix = &quick_fixes(text, item)[0];
        assert_eq!(apply_edits(text, &fix.edits).unwrap(), "int a;\r\nint b;\n");
    }

    #[test]
    fn todo_comments_are_warned_and_collect_is_deduped_sorted() {
        let text = "int a; // TODO fix\n";
        let found = style_diagnostics("a.nx", text);
        assert_eq!(found[0].code, CODE_TODO);
        assert_eq!(found[0].column, 11);
        let all = collect("a.nx", "x // TODO\ny // TODO\n");
        let lines: Vec<usize> = all.iter().map(|d| d.line).collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
        let mut unique = all.clone();
        unique.dedup();
        assert_eq!(unique.len(), all.len());
        let many = "a // TODO\n".repeat(MAX_DIAGNOSTICS + 10);
        assert!(collect("a.nx", &many).len() <= MAX_DIAGNOSTICS);
    }

    #[test]
    fn unknown_identifier_suggests_similar_name() {
        let text = "public class C { public int read() { int counter = 1; return countre; } }";
        let diagnostics = analyze_source("x.nx", text);
        let unknown = diagnostics.iter().find(|d| d.code == "NX1004").unwrap();
        let fixes = quick_fixes(text, unknown);
        assert!(fixes[0].title.contains("counter"));
        let fixed = apply_edits(text, &fixes[0].edits).unwrap();
        assert!(fixed.contains("return counter;"));
        assert!(analyze_source("x.nx", &fixed).is_empty());
    }

    #[test]
    fn missing_semicolon_gets_a_real_fix() {
        let text = "public class C {\n    public int read() {\n        return 1\n    }\n}\n";
        let diagnostics = analyze_source("x.nx", text);
        let item = diagnostics
            .iter()
            .find(|d| d.message.contains("';'"))
            .unwrap_or_else(|| panic!("{diagnostics:?}"));
        let fixes = quick_fixes(text, item);
        let fixed = apply_edits(text, &fixes[0].edits).unwrap();
        assert!(fixed.contains("return 1;\n"), "{fixed}");
        assert!(analyze_source("x.nx", &fixed).is_empty());
    }

    #[test]
    fn semicolon_edit_skips_backwards_over_blank_space() {
        let text = "a = 1\n\n   b";
        let edit = semicolon_edit(text, 3, 4).unwrap();
        assert_eq!(apply_edits(text, &[edit]).unwrap(), "a = 1;\n\n   b");
    }

    #[test]
    fn levenshtein_distances() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("same", "same"), 0);
    }

    #[test]
    fn ide_uses_compiler_diagnostics_and_codes() {
        let diagnostics = analyze_source(
            "src/main.nx",
            "public class C { public int read() { return missing; } }",
        );
        let unknown = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "NX1004")
            .unwrap();
        assert_eq!(unknown.file, "src/main.nx");
        assert!(unknown.help.is_some());
        let rendered = render_excerpt(
            "public class C { public int read() { return missing; } }",
            unknown,
        );
        assert!(rendered.contains("^"));
        assert!(rendered.contains("error[NX1004]"));
    }
}

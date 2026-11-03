//! Inline-Fehlerhervorhebung: Span-Modell, Compiler-Log-Parser und Wellenlinien-Geometrie.

use crate::diagnostics::IdeDiagnostic;

pub const MAX_INLINE_DIAGNOSTICS: usize = 200;
pub const SQUIGGLE_AMPLITUDE: f32 = 2.0;
pub const SQUIGGLE_PERIOD: f32 = 4.0;
pub const ERROR_RGB: [u8; 3] = [0xD6, 0x6C, 0x6C];
pub const WARNING_RGB: [u8; 3] = [0xD9, 0x9B, 0x50];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// `line` und `col_*` sind einsbasiert (Zeichenspalten); `col_end` ist exklusiv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiagnostic {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

impl InlineDiagnostic {
    pub fn from_ide(diagnostic: &IdeDiagnostic) -> Self {
        Self {
            line: diagnostic.line,
            col_start: diagnostic.column,
            col_end: diagnostic.column,
            message: diagnostic.message.clone(),
            severity: if diagnostic.is_error {
                DiagnosticSeverity::Error
            } else {
                DiagnosticSeverity::Warning
            },
        }
    }
}

fn parse_severity(text: &str) -> Option<(DiagnosticSeverity, &str)> {
    for (word, severity) in [
        ("error", DiagnosticSeverity::Error),
        ("warning", DiagnosticSeverity::Warning),
    ] {
        if let Some(rest) = text.strip_prefix(word) {
            return Some((severity, rest));
        }
    }
    None
}

/// `error at line X, col Y..Z: msg` (Z optional, exklusiv).
fn parse_inline_form(rest: &str) -> Option<(usize, usize, usize, String)> {
    let rest = rest.strip_prefix(" at line ")?;
    let (line, rest) = rest.split_once(", col ")?;
    let (span, message) = rest.split_once(':')?;
    let (start, end) = match span.split_once("..") {
        Some((start, end)) => (start.trim().parse().ok()?, end.trim().parse().ok()?),
        None => {
            let start: usize = span.trim().parse().ok()?;
            (start, start)
        }
    };
    Some((
        line.trim().parse().ok()?,
        start,
        end,
        message.trim().to_string(),
    ))
}

/// `error[CODE]: msg` bzw. `warning: msg` (der Ort folgt in einer `-->`-Zeile).
fn parse_header(rest: &str) -> Option<String> {
    let rest = match rest.strip_prefix('[') {
        Some(tail) => tail.split_once(']')?.1,
        None => rest,
    };
    Some(rest.strip_prefix(':')?.trim().to_string())
}

/// `--> pfad:zeile:spalte`; liefert (Pfad, Zeile, Spalte).
fn parse_location(line: &str) -> Option<(&str, usize, usize)> {
    let location = line.trim_start().strip_prefix("-->")?.trim();
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.trim().parse().ok()?;
    let row = parts.next()?.trim().parse().ok()?;
    Some((parts.next()?, row, column))
}

fn same_file(log_path: &str, active: &str) -> bool {
    let normalize = |path: &str| path.replace('\\', "/").to_lowercase();
    let (log_path, active) = (normalize(log_path), normalize(active));
    active.ends_with(&log_path) || log_path.ends_with(&active)
}

/// Liest Compiler-Meldungen aus dem Log. Mit `active_file` werden nur Meldungen
/// für diese Datei übernommen; die Anzahl ist auf [`MAX_INLINE_DIAGNOSTICS`] begrenzt.
pub fn parse_compiler_log(log: &str, active_file: Option<&str>) -> Vec<InlineDiagnostic> {
    let mut found = Vec::new();
    let mut pending: Option<(DiagnosticSeverity, String)> = None;
    for raw in log.lines() {
        if found.len() >= MAX_INLINE_DIAGNOSTICS {
            break;
        }
        let line = raw.trim_start();
        if let Some((severity, rest)) = parse_severity(line) {
            if let Some((row, start, end, message)) = parse_inline_form(rest) {
                found.push(InlineDiagnostic {
                    line: row,
                    col_start: start,
                    col_end: end,
                    message,
                    severity,
                });
                pending = None;
            } else {
                pending = parse_header(rest).map(|message| (severity, message));
            }
        } else if let (Some((severity, message)), Some((path, row, column))) =
            (pending.as_ref(), parse_location(line))
        {
            if active_file.is_none_or(|active| same_file(path, active)) {
                found.push(InlineDiagnostic {
                    line: row,
                    col_start: column,
                    col_end: column,
                    message: message.clone(),
                    severity: *severity,
                });
            }
            pending = None;
        }
    }
    found
}

/// Verwirft Spans außerhalb des Textes und verbreitert leere Spans auf das Token am Start.
pub fn normalize(diagnostics: &mut Vec<InlineDiagnostic>, source: &str) {
    let lines: Vec<&str> = source.split('\n').collect();
    diagnostics.retain_mut(|item| {
        let Some(text) = item.line.checked_sub(1).and_then(|index| lines.get(index)) else {
            return false;
        };
        let text = text.trim_end_matches('\r');
        let length = text.chars().count();
        if item.col_start == 0 || item.col_start > length.max(1) {
            return false;
        }
        if item.col_end <= item.col_start {
            let token = text
                .chars()
                .skip(item.col_start - 1)
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .count();
            item.col_end = item.col_start + token.max(1);
        }
        item.col_end = item.col_end.min(length + 1).max(item.col_start + 1);
        true
    });
    diagnostics.truncate(MAX_INLINE_DIAGNOSTICS);
}

/// Zeichenindex (nicht Byte) von Zeile/Spalte (einsbasiert); `None` außerhalb des Textes.
pub fn char_index(source: &str, line: usize, column: usize) -> Option<usize> {
    let mut offset = 0;
    for (index, text) in source.split('\n').enumerate() {
        let length = text.chars().count();
        if index + 1 == line {
            return (column >= 1 && column - 1 <= length).then(|| offset + column - 1);
        }
        offset += length + 1;
    }
    None
}

/// Zickzack-Punkte von `x0` bis `x1` auf der Grundlinie `y` (nach oben ausgelenkt).
pub fn squiggle_points(x0: f32, x1: f32, y: f32) -> Vec<(f32, f32)> {
    if !x0.is_finite() || !x1.is_finite() || x1 <= x0 {
        return Vec::new();
    }
    let half = SQUIGGLE_PERIOD / 2.0;
    let steps = (((x1 - x0) / half).ceil() as usize).min(4_096);
    (0..=steps)
        .map(|step| {
            let x = (x0 + step as f32 * half).min(x1);
            let offset = if step % 2 == 0 {
                0.0
            } else {
                -SQUIGGLE_AMPLITUDE
            };
            (x, y + offset)
        })
        .collect()
}

/// Führt Live-Analyse und Build-Log zusammen (ohne Duplikate je Zeile/Spalte).
pub fn merge(
    live: &[IdeDiagnostic],
    build: &[InlineDiagnostic],
    source: &str,
) -> Vec<InlineDiagnostic> {
    let mut all: Vec<InlineDiagnostic> = live.iter().map(InlineDiagnostic::from_ide).collect();
    for item in build {
        if !all
            .iter()
            .any(|other| other.line == item.line && other.col_start == item.col_start)
        {
            all.push(item.clone());
        }
    }
    normalize(&mut all, source);
    all
}

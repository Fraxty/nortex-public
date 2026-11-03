use std::fmt::Write;

use super::locale::localize_current;
use super::{Diagnostic, Severity};

pub struct DiagnosticRenderer {
    color: bool,
}

impl DiagnosticRenderer {
    pub fn new(color: bool) -> Self {
        Self { color }
    }

    pub fn render(&self, diagnostic: &Diagnostic, source: Option<&str>) -> String {
        let severity = match diagnostic.severity {
            Severity::Error => ("error", "\x1b[1;31m"),
            Severity::Warning => ("warning", "\x1b[1;33m"),
        };
        let message = localize_current(&diagnostic.message);
        let mut output = String::new();
        if self.color {
            let _ = writeln!(
                output,
                "{}{}[{}]\x1b[0m: {}",
                severity.1, severity.0, diagnostic.code, message
            );
        } else {
            let _ = writeln!(output, "{}[{}]: {}", severity.0, diagnostic.code, message);
        }

        let span = diagnostic.span;
        if span.start.line > 0 {
            let location = diagnostic.file.as_deref().unwrap_or("<source>");
            let blue = if self.color { "\x1b[1;34m" } else { "" };
            let reset = if self.color { "\x1b[0m" } else { "" };
            let _ = writeln!(
                output,
                " {blue}-->{reset} {location}:{}:{}",
                span.start.line, span.start.column
            );

            if let Some(line) =
                source.and_then(|source| source.lines().nth(span.start.line.saturating_sub(1)))
            {
                let line_number_width = span.start.line.to_string().len();
                let _ = writeln!(output, " {:>line_number_width$} {blue}|{reset}", "");
                let _ = writeln!(
                    output,
                    "{:>line_number_width$} {blue}|{reset} {}",
                    span.start.line, line
                );
                let start_column = span.start.column.max(1);
                let line_length = line.chars().count();
                let remaining = line_length
                    .saturating_sub(start_column.saturating_sub(1))
                    .max(1);
                let requested_marker_len = if span.end.line == span.start.line {
                    span.end.column.saturating_sub(start_column).max(1)
                } else {
                    remaining
                };
                let marker_len = requested_marker_len.min(remaining);
                let prefix = line
                    .chars()
                    .take(start_column.saturating_sub(1))
                    .map(|character| if character == '\t' { '\t' } else { ' ' })
                    .collect::<String>();
                let marker = "^".repeat(marker_len);
                let marker_color = if self.color { severity.1 } else { "" };
                let _ = writeln!(
                    output,
                    " {:>line_number_width$} {blue}|{reset} {prefix}{marker_color}{marker}{reset}",
                    ""
                );
            }
        }
        if let Some(help) = &diagnostic.help {
            self.write_help(&mut output, &localize_current(help));
        }
        output
    }

    fn write_help(&self, output: &mut String, help: &str) {
        if self.color {
            let _ = writeln!(output, "\x1b[1;32mhelp\x1b[0m: {help}");
        } else {
            let _ = writeln!(output, "help: {help}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{SourcePosition, SourceSpan};

    fn diagnostic() -> Diagnostic {
        Diagnostic::error_with_code(
            "NX1001",
            "Ungültiger Wert",
            SourceSpan {
                start: SourcePosition {
                    offset: 15,
                    line: 2,
                    column: 9,
                },
                end: SourcePosition {
                    offset: 20,
                    line: 2,
                    column: 14,
                },
            },
        )
        .with_file("src/main.nx")
        .with_help("Verwende einen gültigen Typ.")
    }

    #[test]
    fn renders_location_source_line_marker_and_help_without_color() {
        let rendered = DiagnosticRenderer::new(false).render(
            &diagnostic(),
            Some("int main() {\n    int value = nope;\n}"),
        );

        assert!(rendered.contains("error[NX1001]: Ungültiger Wert"));
        assert!(rendered.contains("--> src/main.nx:2:9"));
        assert!(rendered.contains("int value = nope;"));
        assert!(rendered.contains("    ^^^^^"));
        assert!(rendered.contains("help: Verwende einen gültigen Typ."));
        assert!(!rendered.contains("\x1b["));
    }

    #[test]
    fn renders_color_when_enabled() {
        let rendered =
            DiagnosticRenderer::new(true).render(&diagnostic(), Some("int main() {\n    nope;\n}"));

        assert!(rendered.contains("\x1b[1;31merror[NX1001]\x1b[0m"));
        assert!(rendered.contains("\x1b[1;34m"));
        assert!(rendered.contains("\x1b[1;32mhelp\x1b[0m"));
    }

    #[test]
    fn omits_source_snippet_when_location_is_unavailable() {
        let rendered = DiagnosticRenderer::new(false).render(&diagnostic(), None);
        assert!(rendered.contains("--> src/main.nx:2:9"));
        assert!(rendered.contains("help: Verwende einen gültigen Typ."));
        assert!(!rendered.contains("int value"));
    }

    #[test]
    fn preserves_help_when_span_has_no_location() {
        let mut diagnostic = diagnostic();
        diagnostic.span = SourceSpan::default();
        let rendered = DiagnosticRenderer::new(false).render(&diagnostic, None);

        assert!(rendered.contains("error[NX1001]: Ungültiger Wert"));
        assert!(rendered.contains("help: Verwende einen gültigen Typ."));
        assert!(!rendered.contains("-->"));
    }
}

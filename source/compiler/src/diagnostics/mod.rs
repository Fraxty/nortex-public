//! Source locations and diagnostics shared by compiler stages.

pub mod locale;
pub mod renderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourcePosition {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

/// Verschiebt Positionen hinter einer Textänderung. Die Spalte ändert sich nur
/// in der Zeile, in der die Änderung endet (`anchor_line`, Zeile im alten Text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionShift {
    pub offset: isize,
    pub line: isize,
    pub anchor_line: usize,
    pub column: isize,
}

impl PositionShift {
    pub fn apply(&self, position: &mut SourcePosition) {
        if position.line == self.anchor_line {
            position.column = position.column.saturating_add_signed(self.column);
        }
        position.offset = position.offset.saturating_add_signed(self.offset);
        position.line = position.line.saturating_add_signed(self.line);
    }

    pub fn apply_span(&self, span: &mut SourceSpan) {
        self.apply(&mut span.start);
        self.apply(&mut span.end);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: SourceSpan,
    pub severity: Severity,
    pub file: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            code: "NX0100".to_string(),
            message: message.into(),
            span,
            severity: Severity::Error,
            file: None,
            help: None,
        }
    }

    pub fn error_with_code(
        code: impl Into<String>,
        message: impl Into<String>,
        span: SourceSpan,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            span,
            severity: Severity::Error,
            file: None,
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            code: "NX0004".to_string(),
            message: message.into(),
            span,
            severity: Severity::Warning,
            file: None,
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Compatibility constructor for the original starter project.
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        let position = SourcePosition {
            offset: 0,
            line,
            column,
        };
        Self {
            code: "NX0100".to_string(),
            message: message.into(),
            span: SourceSpan {
                start: position,
                end: position,
            },
            severity: Severity::Error,
            file: None,
            help: None,
        }
    }

    pub fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
    }

    pub fn line(&self) -> usize {
        self.span.start.line
    }
    pub fn column(&self) -> usize {
        self.span.start.column
    }
}

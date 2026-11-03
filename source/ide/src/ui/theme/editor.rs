//! Code editor palette, independent of the general widget theme.

use eframe::egui::Color32;

use super::colors::{Palette, blend};
use crate::editor::highlighting::{HighlightKind, SyntaxClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorPalette {
    pub background: Color32,
    pub text: Color32,
    pub line_number: Color32,
    pub current_line: Color32,
    pub cursor: Color32,
    pub selection: Color32,
    pub error_line: Color32,
    pub keyword: Color32,
    pub number: Color32,
    pub string: Color32,
    pub comment: Color32,
    pub operator: Color32,
    pub type_name: Color32,
    pub function: Color32,
    pub parameter: Color32,
    pub variable: Color32,
    pub modifier: Color32,
}

impl EditorPalette {
    pub fn from_palette(palette: &Palette) -> Self {
        Self {
            type_name: palette.amber,
            function: blend(palette.green, palette.blue, 140),
            parameter: blend(palette.red, palette.text_primary, 70),
            variable: blend(palette.blue, palette.text_primary, 90),
            modifier: blend(palette.amber, palette.red, 120),
            background: palette.app,
            text: palette.text_primary,
            line_number: palette.text_muted,
            current_line: blend(palette.text_primary, palette.app, 14),
            cursor: palette.amber,
            selection: blend(palette.blue, palette.app, 110),
            error_line: blend(palette.red, palette.app, 40),
            keyword: palette.blue,
            number: palette.violet,
            string: palette.green,
            comment: palette.text_muted,
            operator: palette.text_secondary,
        }
    }

    pub fn color_for_class(&self, class: SyntaxClass) -> Color32 {
        match class {
            SyntaxClass::Plain(kind) => self.color_for(kind),
            SyntaxClass::TypeName => self.type_name,
            SyntaxClass::Function => self.function,
            SyntaxClass::Parameter => self.parameter,
            SyntaxClass::Variable => self.variable,
            SyntaxClass::Modifier => self.modifier,
        }
    }

    pub fn color_for(&self, kind: HighlightKind) -> Color32 {
        match kind {
            HighlightKind::Keyword => self.keyword,
            HighlightKind::Identifier => self.text,
            HighlightKind::Number => self.number,
            HighlightKind::String | HighlightKind::Character => self.string,
            HighlightKind::Comment => self.comment,
            HighlightKind::Operator | HighlightKind::Punctuation => self.operator,
        }
    }
}

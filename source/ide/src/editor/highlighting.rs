//! Syntax-Hervorhebung auf Basis des echten NORTEX-Lexers.

use nortex_compiler::lexer::{Lexer, TokenKind};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    Keyword,
    Identifier,
    Number,
    String,
    Character,
    Comment,
    Operator,
    Punctuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub kind: HighlightKind,
}

/// Cache für eine einzelne Dokumentidentität. Eine Änderung der Revision löst
/// genau eine erneute Lex-Auswertung aus; reine Viewport-Neuzeichnungen filtern
/// nur die bereits vorhandenen Spans.
#[derive(Debug, Default)]
pub struct HighlightCache {
    document_id: Option<u64>,
    revision: Option<u64>,
    spans: Vec<HighlightSpan>,
}

impl HighlightCache {
    pub fn update(&mut self, document_id: u64, revision: u64, source: &str) -> &[HighlightSpan] {
        if self.document_id != Some(document_id) || self.revision != Some(revision) {
            self.spans = highlight_nortex(source);
            self.document_id = Some(document_id);
            self.revision = Some(revision);
        }
        &self.spans
    }

    pub fn visible_spans(&self, byte_range: Range<usize>) -> impl Iterator<Item = &HighlightSpan> {
        self.spans
            .iter()
            .filter(move |span| span.end > byte_range.start && span.start < byte_range.end)
    }

    pub fn invalidate(&mut self) {
        self.document_id = None;
        self.revision = None;
        self.spans.clear();
    }
}

pub fn highlight_nortex(source: &str) -> Vec<HighlightSpan> {
    let lexed = Lexer::new(source).tokenize();
    let mut spans: Vec<HighlightSpan> = lexed
        .tokens
        .into_iter()
        .filter_map(|token| {
            let kind = match token.kind {
                TokenKind::Keyword => HighlightKind::Keyword,
                TokenKind::Identifier => HighlightKind::Identifier,
                TokenKind::Number => HighlightKind::Number,
                TokenKind::StringLiteral => HighlightKind::String,
                TokenKind::CharacterLiteral => HighlightKind::Character,
                TokenKind::Symbol(ref symbol) if is_punctuation(symbol) => {
                    HighlightKind::Punctuation
                }
                TokenKind::Symbol(_) => HighlightKind::Operator,
                TokenKind::EndOfFile => return None,
            };
            Some(HighlightSpan {
                start: token.span.start.offset,
                end: token.span.end.offset,
                kind,
            })
        })
        .collect();
    spans.extend(lexed.comments.into_iter().map(|span| HighlightSpan {
        start: span.start.offset,
        end: span.end.offset,
        kind: HighlightKind::Comment,
    }));
    spans.sort_by_key(|span| span.start);
    spans
}

/// Color role of a span in the editor; refines [`HighlightKind`] without changing the
/// token stream used by rename and scope analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxClass {
    Plain(HighlightKind),
    TypeName,
    Function,
    Parameter,
    Variable,
    Modifier,
}

const PRIMITIVE_TYPES: [&str; 12] = [
    "int", "bool", "string", "double", "float", "char", "void", "long", "short", "byte", "uint",
    "ulong",
];

fn is_type_name(text: &str) -> bool {
    PRIMITIVE_TYPES.contains(&text)
        || (text.chars().next().is_some_and(char::is_uppercase)
            && text.chars().any(char::is_lowercase))
}

/// Heuristic, scope-free classification aligned 1:1 with `spans`.
pub fn classify(source: &str, spans: &[HighlightSpan]) -> Vec<SyntaxClass> {
    let text = |index: usize| {
        source
            .get(spans[index].start..spans[index].end)
            .unwrap_or("")
    };
    let significant: Vec<usize> = (0..spans.len())
        .filter(|&index| spans[index].kind != HighlightKind::Comment)
        .collect();
    let mut classes: Vec<SyntaxClass> = spans
        .iter()
        .map(|span| SyntaxClass::Plain(span.kind))
        .collect();
    let mut declared = std::collections::HashMap::<&str, SyntaxClass>::new();
    let mut paren_depth = 0_usize;
    for (position, &index) in significant.iter().enumerate() {
        let word = text(index);
        match spans[index].kind {
            HighlightKind::Punctuation => match word {
                "(" => paren_depth += 1,
                ")" => paren_depth = paren_depth.saturating_sub(1),
                _ => {}
            },
            HighlightKind::Keyword | HighlightKind::Identifier => {
                let previous = position.checked_sub(1).map(|p| significant[p]);
                let next = significant.get(position + 1).copied();
                let next_text = next.map(text);
                if matches!(word, "mut" | "const") {
                    classes[index] = SyntaxClass::Modifier;
                } else if PRIMITIVE_TYPES.contains(&word) {
                    classes[index] = SyntaxClass::TypeName;
                } else if spans[index].kind == HighlightKind::Keyword {
                    continue;
                } else if next_text == Some("(") {
                    classes[index] = SyntaxClass::Function;
                } else if previous.is_some_and(|p| {
                    classes[p] == SyntaxClass::TypeName || text(p) == "]" || text(p) == ">"
                }) {
                    let class = if paren_depth > 0 && matches!(next_text, Some("," | ")")) {
                        SyntaxClass::Parameter
                    } else {
                        SyntaxClass::Variable
                    };
                    declared.insert(word, class);
                    classes[index] = class;
                } else if is_type_name(word) {
                    classes[index] = SyntaxClass::TypeName;
                } else if let Some(class) = declared.get(word) {
                    classes[index] = *class;
                }
            }
            _ => {}
        }
    }
    classes
}

fn is_punctuation(value: &str) -> bool {
    matches!(
        value,
        "{" | "}" | "(" | ")" | "[" | "]" | ";" | "," | "." | ":"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_cache_is_keyed_by_document_and_revision() {
        let mut cache = HighlightCache::default();
        let first = "int alpha = 1;";
        let first_spans = cache.update(10, 1, first).to_vec();
        assert_eq!(cache.update(10, 1, first), first_spans.as_slice());
        assert!(!cache.visible_spans(0..3).collect::<Vec<_>>().is_empty());

        let second = "string beta = \"two\";";
        let second_spans = cache.update(10, 2, second).to_vec();
        assert_ne!(first_spans, second_spans);
        let other_document = cache.update(11, 1, first).to_vec();
        assert_eq!(other_document, first_spans);
        cache.invalidate();
        assert!(cache.visible_spans(0..first.len()).next().is_none());
    }

    #[test]
    fn classification_separates_types_functions_parameters_and_variables() {
        let source = "public int add(int a, int b) { mut int total = a + b; return total; }";
        let spans = highlight_nortex(source);
        let classes = classify(source, &spans);
        let class_of = |needle: &str, nth: usize| {
            let index = spans
                .iter()
                .enumerate()
                .filter(|(_, span)| &source[span.start..span.end] == needle)
                .nth(nth)
                .map(|(index, _)| index)
                .unwrap();
            classes[index]
        };
        assert_eq!(class_of("int", 0), SyntaxClass::TypeName);
        assert_eq!(class_of("add", 0), SyntaxClass::Function);
        assert_eq!(class_of("a", 0), SyntaxClass::Parameter);
        assert_eq!(class_of("a", 1), SyntaxClass::Parameter);
        assert_eq!(class_of("mut", 0), SyntaxClass::Modifier);
        assert_eq!(class_of("total", 0), SyntaxClass::Variable);
        assert_eq!(class_of("total", 1), SyntaxClass::Variable);
    }

    #[test]
    fn syntax_highlighting_uses_compiler_lexer_tokens() {
        let source = "public int answer = 42; string text = \"NORTEX\";";
        let spans = highlight_nortex(source);
        assert!(spans.iter().any(|span| span.kind == HighlightKind::Keyword));
        assert!(spans.iter().any(|span| span.kind == HighlightKind::Number));
        assert!(spans.iter().any(|span| span.kind == HighlightKind::String));
        let with_comment = highlight_nortex("// note\nint x = 1;");
        assert!(
            with_comment
                .iter()
                .any(|span| span.kind == HighlightKind::Comment)
        );
        assert!(spans.iter().all(|span| span.start < span.end
            && source.is_char_boundary(span.start)
            && source.is_char_boundary(span.end)));
    }
}

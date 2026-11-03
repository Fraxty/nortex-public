//! Case-sensitive ASCII lexer for the NORTEX language.
//!
//! Identifiers are intentionally ASCII-only. Unicode is accepted in comments
//! and string literals. The lexer does not normalize identifier casing.

use crate::diagnostics::{Diagnostic, SourcePosition, SourceSpan};

pub mod incremental;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Keyword,
    Number,
    StringLiteral,
    CharacterLiteral,
    Symbol(String),
    EndOfFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
    /// Kommentarbereiche für IDE-Hervorhebung; Parser erhalten weiterhin nur Tokens.
    pub comments: Vec<SourceSpan>,
}

pub struct Lexer<'a> {
    source: &'a str,
    position: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
    comments: Vec<SourceSpan>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            comments: Vec::new(),
        }
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn tokenize(mut self) -> LexOutput {
        while !self.is_at_end() {
            self.skip_whitespace_and_comments();
            if self.is_at_end() {
                break;
            }
            self.scan_token();
        }
        self.finish()
    }

    fn finish(mut self) -> LexOutput {
        let position = self.current_position();
        self.tokens.push(Token {
            kind: TokenKind::EndOfFile,
            lexeme: String::new(),
            span: SourceSpan {
                start: position,
                end: position,
            },
        });

        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
            comments: self.comments,
        }
    }

    fn scan_token(&mut self) {
        let start = self.current_position();
        let start_offset = self.position;
        let Some(ch) = self.peek_char() else {
            return;
        };

        if ch.is_ascii_alphabetic() || ch == '_' {
            self.advance_char();
            while matches!(self.peek_char(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
                self.advance_char();
            }
            let lexeme = &self.source[start_offset..self.position];
            let kind = if is_keyword(lexeme) {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            self.push_token(kind, start_offset, start);
            return;
        }

        if ch.is_ascii_digit() {
            self.scan_number();
            self.push_token(TokenKind::Number, start_offset, start);
            return;
        }

        if ch == '\'' {
            self.scan_character_literal(start);
            self.push_token(TokenKind::CharacterLiteral, start_offset, start);
            return;
        }

        if ch == '"' {
            self.advance_char();
            let mut terminated = false;
            while let Some(current) = self.peek_char() {
                if current == '"' {
                    self.advance_char();
                    terminated = true;
                    break;
                }
                if current == '\\' {
                    self.advance_char();
                    match self.peek_char() {
                        Some('n' | 'r' | 't' | '0' | '"' | '\\') => {
                            self.advance_char();
                        }
                        Some(other) => {
                            self.advance_char();
                            let span = SourceSpan {
                                start,
                                end: self.current_position(),
                            };
                            self.diagnostics.push(Diagnostic::error(
                                format!("Unbekannte Escape-Sequenz: \\{other}"),
                                span,
                            ));
                        }
                        None => break,
                    }
                } else if current == '\n' {
                    let span = SourceSpan {
                        start,
                        end: self.current_position(),
                    };
                    self.diagnostics.push(Diagnostic::error(
                        "Zeilenumbruch in String-Literal nicht erlaubt",
                        span,
                    ));
                    break;
                } else {
                    self.advance_char();
                }
            }
            if !terminated {
                let span = SourceSpan {
                    start,
                    end: self.current_position(),
                };
                self.diagnostics.push(Diagnostic::error(
                    "Nicht abgeschlossenes String-Literal",
                    span,
                ));
            }
            self.push_token(TokenKind::StringLiteral, start_offset, start);
            return;
        }

        // Longest-match operator recognition.
        let three = self
            .peek_three_chars()
            .filter(|text| matches!(text.as_str(), "<<=" | ">>="));
        let two = self.peek_two_chars().filter(|text| {
            matches!(
                text.as_str(),
                "==" | "!="
                    | "<="
                    | ">="
                    | "<<"
                    | ">>"
                    | "&&"
                    | "||"
                    | "++"
                    | "--"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | "->"
                    | "=>"
                    | "::"
            )
        });
        let operator = three.or(two).unwrap_or_else(|| ch.to_string());
        let valid = matches!(
            operator.as_str(),
            "{" | "}"
                | "("
                | ")"
                | "["
                | "]"
                | ";"
                | ","
                | "."
                | ":"
                | "<"
                | ">"
                | "="
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "!"
                | "&"
                | "|"
                | "^"
                | "~"
                | "?"
                | "=="
                | "!="
                | "<="
                | ">="
                | "&&"
                | "||"
                | "++"
                | "--"
                | "+="
                | "-="
                | "*="
                | "/="
                | "%="
                | "&="
                | "|="
                | "^="
                | "<<="
                | ">>="
                | "<<"
                | ">>"
                | "->"
                | "=>"
                | "::"
        );
        if valid {
            for _ in operator.chars() {
                self.advance_char();
            }
            self.tokens.push(Token {
                kind: TokenKind::Symbol(operator.clone()),
                lexeme: operator,
                span: SourceSpan {
                    start,
                    end: self.current_position(),
                },
            });
        } else {
            self.advance_char();
            let span = SourceSpan {
                start,
                end: self.current_position(),
            };
            let message = if !ch.is_ascii() {
                format!("Unicode-Zeichen '{ch}' ist in Bezeichnern und Operatoren nicht zulässig")
            } else {
                format!("Unbekanntes Zeichen '{ch}'")
            };
            self.diagnostics.push(Diagnostic::error(message, span));
        }
    }

    fn scan_number(&mut self) {
        // Hexadecimal and binary integer literals.
        if self.peek_char() == Some('0') && matches!(self.peek_next_char(), Some('x' | 'X')) {
            self.advance_char();
            self.advance_char();
            let digit_start = self.position;
            while matches!(self.peek_char(), Some(c) if c.is_ascii_hexdigit() || c == '_') {
                self.advance_char();
            }
            if self.position == digit_start {
                let span = SourceSpan {
                    start: self.current_position(),
                    end: self.current_position(),
                };
                self.diagnostics.push(Diagnostic::error(
                    "Hexadezimales Literal benötigt mindestens eine Ziffer.",
                    span,
                ));
            }
        } else if self.peek_char() == Some('0') && matches!(self.peek_next_char(), Some('b' | 'B'))
        {
            self.advance_char();
            self.advance_char();
            let digit_start = self.position;
            while matches!(self.peek_char(), Some('0' | '1' | '_')) {
                self.advance_char();
            }
            if self.position == digit_start {
                let span = SourceSpan {
                    start: self.current_position(),
                    end: self.current_position(),
                };
                self.diagnostics.push(Diagnostic::error(
                    "Binäres Literal benötigt mindestens eine 0- oder 1-Ziffer.",
                    span,
                ));
            }
            if matches!(self.peek_char(), Some(c) if c.is_ascii_digit() && c != '0' && c != '1') {
                let span = SourceSpan {
                    start: self.current_position(),
                    end: self.current_position(),
                };
                self.diagnostics.push(Diagnostic::error(
                    "Ein Binärliteral darf nur die Ziffern 0 und 1 enthalten.",
                    span,
                ));
                while matches!(self.peek_char(), Some(c) if c.is_ascii_digit() || c == '_') {
                    self.advance_char();
                }
            }
        } else {
            while matches!(self.peek_char(), Some(c) if c.is_ascii_digit() || c == '_') {
                self.advance_char();
            }
            if self.peek_char() == Some('.')
                && self.peek_next_char().is_some_and(|c| c.is_ascii_digit())
            {
                self.advance_char();
                while matches!(self.peek_char(), Some(c) if c.is_ascii_digit() || c == '_') {
                    self.advance_char();
                }
            }
            if matches!(self.peek_char(), Some('e' | 'E')) {
                self.advance_char();
                if matches!(self.peek_char(), Some('+' | '-')) {
                    self.advance_char();
                }
                let exponent_start = self.position;
                while matches!(self.peek_char(), Some(c) if c.is_ascii_digit() || c == '_') {
                    self.advance_char();
                }
                if self.position == exponent_start {
                    let span = SourceSpan {
                        start: self.current_position(),
                        end: self.current_position(),
                    };
                    self.diagnostics.push(Diagnostic::error(
                        "Exponent benötigt mindestens eine Ziffer.",
                        span,
                    ));
                }
            }
        }
        // Numeric suffixes are lexed as part of the literal and validated by
        // the type checker. Supported spellings are documented centrally.
        while matches!(self.peek_char(), Some(c) if c.is_ascii_alphabetic()) {
            self.advance_char();
        }
    }

    fn scan_character_literal(&mut self, start: SourcePosition) {
        self.advance_char(); // opening quote
        let mut values = 0usize;
        if self.peek_char() == Some('\\') {
            self.advance_char();
            if self.peek_char().is_some() {
                self.advance_char();
                values = 1;
            }
        } else if self.peek_char().is_some_and(|c| c != '\'' && c != '\n') {
            self.advance_char();
            values = 1;
        }
        let closed = self.peek_char() == Some('\'');
        if closed {
            self.advance_char();
        }
        if !closed || values != 1 {
            let span = SourceSpan {
                start,
                end: self.current_position(),
            };
            self.diagnostics.push(Diagnostic::error("Zeichenliteral muss genau einen Unicode-Skalarwert enthalten und mit einem Apostroph geschlossen werden.", span));
        }
    }

    fn peek_three_chars(&self) -> Option<String> {
        let mut chars = self.source.get(self.position..)?.chars();
        Some(format!(
            "{}{}{}",
            chars.next()?,
            chars.next()?,
            chars.next()?
        ))
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                Some(c) if c.is_whitespace() => {
                    self.advance_char();
                }
                Some('/') if self.peek_next_char() == Some('/') => {
                    let start = self.current_position();
                    while let Some(c) = self.peek_char() {
                        self.advance_char();
                        if c == '\n' {
                            break;
                        }
                    }
                    self.comments.push(SourceSpan {
                        start,
                        end: self.current_position(),
                    });
                }
                Some('/') if self.peek_next_char() == Some('*') => {
                    let start = self.current_position();
                    self.advance_char();
                    self.advance_char();
                    let mut closed = false;
                    while !self.is_at_end() {
                        if self.peek_char() == Some('*') && self.peek_next_char() == Some('/') {
                            self.advance_char();
                            self.advance_char();
                            closed = true;
                            break;
                        }
                        self.advance_char();
                    }
                    let span = SourceSpan {
                        start,
                        end: self.current_position(),
                    };
                    self.comments.push(span);
                    if !closed {
                        self.diagnostics.push(Diagnostic::error(
                            "Nicht abgeschlossener Blockkommentar",
                            span,
                        ));
                    }
                }
                _ => break,
            }
        }
    }

    fn push_token(&mut self, kind: TokenKind, start_offset: usize, start: SourcePosition) {
        self.tokens.push(Token {
            kind,
            lexeme: self.source[start_offset..self.position].to_string(),
            span: SourceSpan {
                start,
                end: self.current_position(),
            },
        });
    }

    fn current_position(&self) -> SourcePosition {
        SourcePosition {
            offset: self.position,
            line: self.line,
            column: self.column,
        }
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.position += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn peek_char(&self) -> Option<char> {
        self.source.get(self.position..)?.chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut chars = self.source.get(self.position..)?.chars();
        chars.next()?;
        chars.next()
    }

    fn peek_two_chars(&self) -> Option<String> {
        let mut chars = self.source.get(self.position..)?.chars();
        Some(format!("{}{}", chars.next()?, chars.next()?))
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.source.len()
    }
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "using"
            | "namespace"
            | "class"
            | "struct"
            | "record"
            | "interface"
            | "trait"
            | "enum"
            | "public"
            | "private"
            | "protected"
            | "internal"
            | "static"
            | "abstract"
            | "sealed"
            | "partial"
            | "extern"
            | "unsafe"
            | "const"
            | "virtual"
            | "override"
            | "async"
            | "readonly"
            | "return"
            | "throw"
            | "if"
            | "else"
            | "while"
            | "for"
            | "in"
            | "switch"
            | "case"
            | "default"
            | "break"
            | "continue"
            | "try"
            | "catch"
            | "finally"
            | "match"
            | "true"
            | "false"
            | "null"
            | "new"
            | "await"
            | "this"
            | "base"
            | "operator"
            | "var"
            | "mut"
            | "ref"
            | "out"
            | "params"
            | "is"
            | "as"
            | "typeof"
            | "sizeof"
            | "where"
            | "let"
            | "sbyte"
            | "short"
            | "int"
            | "long"
            | "i128"
            | "byte"
            | "ushort"
            | "uint"
            | "ulong"
            | "u128"
            | "nint"
            | "nuint"
            | "isize"
            | "usize"
            | "float"
            | "double"
            | "decimal"
            | "bool"
            | "char"
            | "string"
            | "void"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_case_sensitive() {
        let output = Lexer::new("name Name NAME").tokenize();
        let names: Vec<_> = output
            .tokens
            .iter()
            .take(3)
            .map(|token| token.lexeme.as_str())
            .collect();
        assert_eq!(names, vec!["name", "Name", "NAME"]);
    }

    #[test]
    fn comments_are_reported_separately_without_changing_parser_tokens() {
        let output = Lexer::new("// heading\nint value = 1; /* body */").tokenize();
        assert_eq!(output.comments.len(), 2);
        assert!(output.tokens.iter().any(|token| token.lexeme == "int"));
        assert!(
            !output
                .tokens
                .iter()
                .any(|token| token.lexeme == "heading" || token.lexeme == "body")
        );
    }

    #[test]
    fn unicode_is_allowed_inside_strings() {
        let output = Lexer::new("\"Grüße\"").tokenize();
        assert!(output.diagnostics.is_empty());
        assert_eq!(output.tokens[0].kind, TokenKind::StringLiteral);
    }
    #[test]
    fn keywords_and_characters_have_distinct_tokens() {
        let output = Lexer::new("class charValue = 'A'; int value = 1;").tokenize();
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        assert_eq!(output.tokens[0].kind, TokenKind::Keyword);
        assert!(
            output
                .tokens
                .iter()
                .any(|token| token.kind == TokenKind::CharacterLiteral)
        );
        assert!(
            output
                .tokens
                .iter()
                .any(|token| token.lexeme == "int" && token.kind == TokenKind::Keyword)
        );
    }

    #[test]
    fn numeric_forms_and_compound_operators_are_lexed() {
        let output = Lexer::new("0xFF 0b1010 10L 1.5e3 value %= 2; value <<= 1;").tokenize();
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let lexemes: Vec<_> = output
            .tokens
            .iter()
            .map(|token| token.lexeme.as_str())
            .collect();
        for expected in ["0xFF", "0b1010", "10L", "1.5e3", "%=", "<<="] {
            assert!(
                lexemes.contains(&expected),
                "missing token {expected}: {lexemes:?}"
            );
        }
    }

    #[test]
    fn bitwise_assignments_are_single_tokens_and_get_set_are_identifiers() {
        let output = Lexer::new("value &= 1; value |= 2; value ^= 3; get set let").tokenize();
        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let lexemes: Vec<_> = output.tokens.iter().collect();
        for expected in ["&=", "|=", "^="] {
            assert!(lexemes.iter().any(
                |token| token.lexeme == expected && matches!(&token.kind, TokenKind::Symbol(_))
            ));
        }
        assert!(
            lexemes
                .iter()
                .any(|token| token.lexeme == "get" && token.kind == TokenKind::Identifier)
        );
        assert!(
            lexemes
                .iter()
                .any(|token| token.lexeme == "set" && token.kind == TokenKind::Identifier)
        );
        assert!(
            lexemes
                .iter()
                .any(|token| token.lexeme == "let" && token.kind == TokenKind::Keyword)
        );
        let intrinsic_output = Lexer::new("intrinsic").tokenize();
        assert_eq!(intrinsic_output.tokens[0].kind, TokenKind::Identifier);
    }
}

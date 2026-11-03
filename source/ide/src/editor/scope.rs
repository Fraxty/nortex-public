//! Umschließende Scopes (Struct, Klasse, Funktion, ...) für die Breadcrumb-Zeile; token-basiert.

use super::highlighting::{HighlightKind, highlight_nortex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Struct,
    Class,
    Enum,
    Interface,
    Trait,
    Impl,
    Namespace,
    Module,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    pub name: String,
    /// Nullbasierte Zeile der Deklaration und der schließenden Klammer.
    pub start_line: usize,
    pub end_line: usize,
}

impl Scope {
    pub fn label(&self) -> String {
        match self.kind {
            ScopeKind::Function => format!("fn {}()", self.name),
            ScopeKind::Struct => format!("struct {}", self.name),
            ScopeKind::Class => format!("class {}", self.name),
            ScopeKind::Enum => format!("enum {}", self.name),
            ScopeKind::Interface => format!("interface {}", self.name),
            ScopeKind::Trait => format!("trait {}", self.name),
            ScopeKind::Impl => format!("impl {}", self.name),
            ScopeKind::Namespace => format!("namespace {}", self.name),
            ScopeKind::Module => format!("module {}", self.name),
        }
    }
}

fn type_kind(word: &str) -> Option<ScopeKind> {
    Some(match word {
        "struct" => ScopeKind::Struct,
        "class" => ScopeKind::Class,
        "enum" => ScopeKind::Enum,
        "interface" => ScopeKind::Interface,
        "trait" => ScopeKind::Trait,
        "impl" => ScopeKind::Impl,
        "namespace" => ScopeKind::Namespace,
        "module" => ScopeKind::Module,
        _ => return None,
    })
}

/// Wörter, hinter denen `name(` ein Aufruf und keine Deklaration ist.
fn is_call_context(word: &str) -> bool {
    matches!(
        word,
        "return" | "new" | "await" | "throw" | "else" | "in" | "is" | "as" | "using" | "case"
    )
}

/// Alle Scopes des Textes, sortiert nach Beginn.
pub fn scopes(text: &str) -> Vec<Scope> {
    let mut line_starts = vec![0usize];
    line_starts.extend(text.match_indices('\n').map(|(index, _)| index + 1));
    let line_of = |byte: usize| {
        line_starts
            .partition_point(|&start| start <= byte)
            .saturating_sub(1)
    };

    let tokens: Vec<_> = highlight_nortex(text)
        .into_iter()
        .filter(|span| {
            !matches!(
                span.kind,
                HighlightKind::Comment | HighlightKind::String | HighlightKind::Character
            )
        })
        .collect();
    let word = |index: usize| &text[tokens[index].start..tokens[index].end];

    let mut stack: Vec<Option<Scope>> = Vec::new();
    let mut pending: Option<(ScopeKind, String, usize)> = None;
    let mut found = Vec::new();
    for index in 0..tokens.len() {
        let token = &tokens[index];
        let current = word(index);
        match token.kind {
            HighlightKind::Keyword | HighlightKind::Identifier => {
                if let Some(kind) = type_kind(current)
                    && let Some(next) = tokens.get(index + 1)
                    && next.kind == HighlightKind::Identifier
                {
                    pending = Some((kind, word(index + 1).to_owned(), line_of(next.start)));
                } else if token.kind == HighlightKind::Identifier
                    && tokens.get(index + 1).is_some_and(|next| {
                        word(index + 1) == "(" && next.kind == HighlightKind::Punctuation
                    })
                    && index > 0
                    && matches!(
                        tokens[index - 1].kind,
                        HighlightKind::Identifier | HighlightKind::Keyword
                    )
                    && !is_call_context(word(index - 1))
                {
                    pending = Some((
                        ScopeKind::Function,
                        current.to_owned(),
                        line_of(token.start),
                    ));
                }
            }
            HighlightKind::Punctuation | HighlightKind::Operator => match current {
                "{" => stack.push(pending.take().map(|(kind, name, start_line)| Scope {
                    kind,
                    name,
                    start_line,
                    end_line: start_line,
                })),
                "}" => {
                    if let Some(Some(mut scope)) = stack.pop() {
                        scope.end_line = line_of(token.start);
                        found.push(scope);
                    }
                }
                ";" => pending = None,
                _ => {}
            },
            _ => {}
        }
    }
    // Unterbrochener Code (fehlende `}`): offene Scopes reichen bis zum Dateiende.
    let last_line = line_starts.len() - 1;
    for mut scope in stack.into_iter().flatten() {
        scope.end_line = last_line;
        found.push(scope);
    }
    found.sort_by_key(|scope| (scope.start_line, std::cmp::Reverse(scope.end_line)));
    found
}

/// Verschachtelte Scopes um `line` (nullbasiert), äußerster zuerst.
pub fn scope_path(text: &str, line: usize) -> Vec<Scope> {
    scopes(text)
        .into_iter()
        .filter(|scope| scope.start_line <= line && line <= scope.end_line)
        .collect()
}

/// Nullbasierte Zeile eines Zeichenindex.
pub fn line_of_char(text: &str, char_index: usize) -> usize {
    text.chars().take(char_index).filter(|&c| c == '\n').count()
}

/// Zeichenindex des Zeilenanfangs (nullbasierte Zeile).
pub fn char_index_of_line(text: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let mut seen = 0;
    for (index, c) in text.chars().enumerate() {
        if c == '\n' {
            seen += 1;
            if seen == line {
                return index + 1;
            }
        }
    }
    text.chars().count()
}

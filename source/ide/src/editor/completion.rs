//! Context-aware completion over the current NORTEX document.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use nortex_compiler::ast::{
    self, BlockStatement, Declaration, FunctionDeclaration, MemberDeclaration, Program, Statement,
};
use nortex_compiler::diagnostics::SourceSpan;
use nortex_compiler::lexer::{Token, TokenKind};

use super::analysis::{Phase, analyze_active_document, timed};
use super::imports::{ImportedModule, resolve_imports};
use super::types::{Member, Scope, TypeEnv, type_text};

pub const MAX_SUGGESTIONS: usize = 12;
pub const MIN_PREFIX_CHARS: usize = 2;
const MAX_CACHED_MODULES: usize = 500;

pub const KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "else",
    "enum",
    "extern",
    "false",
    "finally",
    "float",
    "for",
    "if",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "isize",
    "let",
    "long",
    "match",
    "mut",
    "namespace",
    "new",
    "nint",
    "nuint",
    "null",
    "operator",
    "out",
    "override",
    "params",
    "partial",
    "private",
    "protected",
    "public",
    "readonly",
    "record",
    "ref",
    "return",
    "sbyte",
    "sealed",
    "short",
    "sizeof",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "trait",
    "true",
    "try",
    "typeof",
    "u128",
    "uint",
    "ulong",
    "unsafe",
    "ushort",
    "using",
    "usize",
    "var",
    "virtual",
    "void",
    "where",
    "while",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Variable,
    Constant,
    Function,
    Method,
    Class,
    Type,
    Module,
}

impl CompletionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Keyword => "Schlüsselwort",
            Self::Variable => "Variable",
            Self::Constant => "Konstante",
            Self::Function => "Funktion",
            Self::Method => "Methode",
            Self::Class => "Klasse",
            Self::Type => "Typ",
            Self::Module => "Modul",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionDetail {
    pub kind: CompletionKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub signature: String,
    pub active_parameter: Option<(usize, String)>,
}

#[derive(Debug, Clone)]
struct Symbol {
    name: String,
    kind: CompletionKind,
    detail: String,
    type_name: Option<String>,
    span: SourceSpan,
}

struct CachedModuleSymbols {
    program: Arc<Program>,
    symbols: Arc<Vec<Symbol>>,
}

static MODULE_SYMBOLS: OnceLock<Mutex<HashMap<PathBuf, CachedModuleSymbols>>> = OnceLock::new();

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Wortpräfix links vom Cursor (Zeichenindex); `None`, wenn zu kurz oder ungültig.
pub fn prefix_at(chars: &[char], cursor: usize) -> Option<String> {
    let cursor = cursor.min(chars.len());
    let start = chars[..cursor]
        .iter()
        .rposition(|c| !is_word(*c))
        .map_or(0, |index| index + 1);
    let prefix: String = chars[start..cursor].iter().collect();
    let first = prefix.chars().next()?;
    (prefix.chars().count() >= MIN_PREFIX_CHARS && (first.is_alphabetic() || first == '_'))
        .then_some(prefix)
}

fn byte_offset(chars: &[char], cursor: usize) -> usize {
    chars
        .iter()
        .take(cursor.min(chars.len()))
        .map(|ch| ch.len_utf8())
        .sum()
}

fn is_inside(span: SourceSpan, offset: usize) -> bool {
    span.start.offset <= offset && offset <= span.end.offset
}

fn signature(function: &FunctionDeclaration) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, type_text(&parameter.type_ref)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}({parameters}) -> {}",
        function.name,
        type_text(&function.return_type)
    )
}

fn push_symbol(symbols: &mut Vec<Symbol>, symbol: Symbol) {
    if !symbols.iter().any(|existing| existing.name == symbol.name) {
        symbols.push(symbol);
    }
}

/// Typauflösung für Initialisierer: Typumgebung aus Import-Graph und aktiver Datei
/// plus die Klasse, in der der Cursor steht.
struct Inference<'a> {
    env: &'a TypeEnv<'a>,
    class: Option<&'a str>,
}

impl Inference<'_> {
    fn type_of(&self, expression: &ast::Expression, locals: &[Symbol]) -> Option<String> {
        let lookup = |name: &str| local_type(locals, name);
        let scope = Scope {
            locals: &lookup,
            class: self.class,
        };
        self.env.expression_type(expression, &scope)
    }
}

fn local_type(locals: &[Symbol], name: &str) -> Option<Option<String>> {
    locals
        .iter()
        .find(|symbol| {
            symbol.name == name
                && matches!(
                    symbol.kind,
                    CompletionKind::Variable | CompletionKind::Constant
                )
        })
        .map(|symbol| symbol.type_name.clone())
}

fn collect_statement_symbols(
    statement: &Statement,
    offset: usize,
    inference: &Inference,
    symbols: &mut Vec<Symbol>,
) {
    match statement {
        Statement::Variable(variable) if variable.span.start.offset <= offset => {
            let type_name = variable.type_ref.as_ref().map(type_text).or_else(|| {
                variable
                    .initializer
                    .as_ref()
                    .and_then(|expression| inference.type_of(expression, symbols))
            });
            let kind = if variable.mutability == ast::VariableMutability::CompileTime {
                CompletionKind::Constant
            } else {
                CompletionKind::Variable
            };
            push_symbol(
                symbols,
                Symbol {
                    name: variable.name.clone(),
                    kind,
                    detail: type_name.as_ref().map_or_else(
                        || kind.label().to_owned(),
                        |ty| format!("{}: {ty}", kind.label()),
                    ),
                    type_name,
                    span: variable.span,
                },
            );
        }
        Statement::Block(block) if is_inside(block.span, offset) => {
            collect_block_symbols(block, offset, inference, symbols);
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            if statement_span(then_branch).is_some_and(|span| is_inside(span, offset)) {
                collect_statement_symbols(then_branch, offset, inference, symbols);
            }
            if let Some(branch) = else_branch
                && statement_span(branch).is_some_and(|span| is_inside(span, offset))
            {
                collect_statement_symbols(branch, offset, inference, symbols);
            }
        }
        Statement::While { body, .. } | Statement::ForEach { body, .. } => {
            if statement_span(body).is_some_and(|span| is_inside(span, offset)) {
                collect_statement_symbols(body, offset, inference, symbols);
            }
            if let Statement::ForEach {
                variable_name,
                variable_type,
                span,
                ..
            } = statement
                && is_inside(*span, offset)
            {
                let type_name = type_text(variable_type);
                push_symbol(
                    symbols,
                    Symbol {
                        name: variable_name.clone(),
                        kind: CompletionKind::Variable,
                        detail: format!("Variable: {type_name}"),
                        type_name: Some(type_name),
                        span: *span,
                    },
                );
            }
        }
        Statement::For {
            initializer,
            body,
            span,
            ..
        } => {
            if is_inside(*span, offset) {
                if let Some(initializer) = initializer {
                    collect_statement_symbols(initializer, offset, inference, symbols);
                }
                if statement_span(body).is_some_and(|body_span| is_inside(body_span, offset)) {
                    collect_statement_symbols(body, offset, inference, symbols);
                }
            }
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            if is_inside(body.span, offset) {
                collect_block_symbols(body, offset, inference, symbols);
            }
            for catch in catches.iter().filter(|catch| is_inside(catch.span, offset)) {
                if let Some(name) = &catch.variable_name {
                    let type_name = catch.exception_type.as_ref().map(type_text);
                    push_symbol(
                        symbols,
                        Symbol {
                            name: name.clone(),
                            kind: CompletionKind::Variable,
                            detail: type_name
                                .as_ref()
                                .map_or_else(|| "Variable".into(), |ty| format!("Variable: {ty}")),
                            type_name,
                            span: catch.span,
                        },
                    );
                }
                collect_block_symbols(&catch.body, offset, inference, symbols);
            }
            if let Some(block) = finally_block
                && is_inside(block.span, offset)
            {
                collect_block_symbols(block, offset, inference, symbols);
            }
        }
        Statement::Switch { cases, .. } => {
            for case in cases.iter().filter(|case| is_inside(case.span, offset)) {
                for nested in &case.statements {
                    collect_statement_symbols(nested, offset, inference, symbols);
                }
            }
        }
        Statement::Match { arms, .. } => {
            for arm in arms.iter().filter(|arm| is_inside(arm.span, offset)) {
                if let Some(binding) = &arm.binding {
                    push_symbol(
                        symbols,
                        Symbol {
                            name: binding.clone(),
                            kind: CompletionKind::Variable,
                            detail: "Pattern-Bindung".into(),
                            type_name: None,
                            span: arm.span,
                        },
                    );
                }
                collect_block_symbols(&arm.body, offset, inference, symbols);
            }
        }
        _ => {}
    }
}

fn statement_span(statement: &Statement) -> Option<SourceSpan> {
    match statement {
        Statement::Block(value) => Some(value.span),
        Statement::Variable(value) => Some(value.span),
        Statement::Return(_, span)
        | Statement::Throw(_, span)
        | Statement::Break(span)
        | Statement::Continue(span)
        | Statement::Expression(_, span)
        | Statement::Empty(span) => Some(*span),
        Statement::If { span, .. }
        | Statement::While { span, .. }
        | Statement::For { span, .. }
        | Statement::ForEach { span, .. }
        | Statement::Switch { span, .. }
        | Statement::Try { span, .. }
        | Statement::Match { span, .. } => Some(*span),
    }
}

fn collect_block_symbols(
    block: &BlockStatement,
    offset: usize,
    inference: &Inference,
    symbols: &mut Vec<Symbol>,
) {
    for statement in &block.statements {
        collect_statement_symbols(statement, offset, inference, symbols);
    }
}

/// Parameter und lokale Deklarationen der Funktion oder Methode, in deren Rumpf `offset` liegt.
fn collect_function_locals(
    function: &FunctionDeclaration,
    offset: usize,
    inference: &Inference,
    symbols: &mut Vec<Symbol>,
) {
    let Some(body) = &function.body else {
        return;
    };
    if !is_inside(body.span, offset) {
        return;
    }
    for parameter in &function.parameters {
        let type_name = type_text(&parameter.type_ref);
        push_symbol(
            symbols,
            Symbol {
                name: parameter.name.clone(),
                kind: CompletionKind::Variable,
                detail: format!("Parameter: {type_name}"),
                type_name: Some(type_name),
                span: parameter.span,
            },
        );
    }
    collect_block_symbols(body, offset, inference, symbols);
}

/// Parameter und lokale Variablen am Cursor; sie verdecken gleichnamige Deklarationen.
fn local_symbols(program: &Program, offset: usize, inference: &Inference) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                collect_function_locals(function, offset, inference, &mut symbols);
            }
            Declaration::Class(class) if is_inside(class.span, offset) => {
                for member in &class.members {
                    if let MemberDeclaration::Method(method) = member {
                        collect_function_locals(method, offset, inference, &mut symbols);
                    }
                }
            }
            _ => {}
        }
    }
    symbols
}

/// Deklarationen der Datei: Funktionen, globale Variablen, Typen und – nur wenn der
/// Cursor in der Klasse steht – deren Member. Für importierte Module ist `offset`
/// `usize::MAX`, sie liefern daher nur die Deklarationen der obersten Ebene.
fn declaration_symbols(program: &Program, offset: usize) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                push_symbol(
                    &mut symbols,
                    Symbol {
                        name: function.name.clone(),
                        kind: CompletionKind::Function,
                        detail: signature(function),
                        type_name: Some(type_text(&function.return_type)),
                        span: function.span,
                    },
                );
            }
            Declaration::GlobalVariable(variable) if variable.span.start.offset <= offset => {
                let type_name = type_text(&variable.type_ref);
                push_symbol(
                    &mut symbols,
                    Symbol {
                        name: variable.name.clone(),
                        kind: CompletionKind::Variable,
                        detail: format!("Variable: {type_name}"),
                        type_name: Some(type_name),
                        span: variable.span,
                    },
                );
            }
            Declaration::Class(class) => {
                push_symbol(
                    &mut symbols,
                    Symbol {
                        name: class.name.clone(),
                        kind: CompletionKind::Class,
                        detail: match class.kind {
                            ast::TypeDeclarationKind::Class => "Klasse".into(),
                            ast::TypeDeclarationKind::Struct => "Struktur".into(),
                            ast::TypeDeclarationKind::Record => "Record".into(),
                            ast::TypeDeclarationKind::Interface => "Schnittstelle".into(),
                            ast::TypeDeclarationKind::Trait => "Trait".into(),
                            ast::TypeDeclarationKind::Enum => "Aufzählung".into(),
                        },
                        type_name: None,
                        span: class.span,
                    },
                );
                if is_inside(class.span, offset) {
                    for member in &class.members {
                        match member {
                            MemberDeclaration::Field(field) => {
                                let type_name = type_text(&field.type_ref);
                                push_symbol(
                                    &mut symbols,
                                    Symbol {
                                        name: field.name.clone(),
                                        kind: CompletionKind::Variable,
                                        detail: format!("Feld: {type_name}"),
                                        type_name: Some(type_name),
                                        span: field.span,
                                    },
                                );
                            }
                            MemberDeclaration::Method(method) => push_symbol(
                                &mut symbols,
                                Symbol {
                                    name: method.name.clone(),
                                    kind: CompletionKind::Method,
                                    detail: signature(method),
                                    type_name: Some(type_text(&method.return_type)),
                                    span: method.span,
                                },
                            ),
                            MemberDeclaration::Variant(variant) => push_symbol(
                                &mut symbols,
                                Symbol {
                                    name: variant.name.clone(),
                                    kind: CompletionKind::Constant,
                                    detail: format!("{}-Variante", class.name),
                                    type_name: Some(class.name.clone()),
                                    span: variant.span,
                                },
                            ),
                            MemberDeclaration::Operator(_) => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    symbols
}

fn enclosing_class(program: &Program, offset: usize) -> Option<&str> {
    program
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            Declaration::Class(class) if is_inside(class.span, offset) => Some(class.name.as_str()),
            _ => None,
        })
}

/// Deklarationssymbole eines importierten Moduls. Das Ergebnis hängt nur vom geparsten
/// Programm ab und wird pro Pfad behalten, solange der Modul-Cache dasselbe `Program`
/// liefert; ändert sich die Datei, entsteht ein neues `Arc` und der Eintrag wird ersetzt.
fn module_symbols(module: &ImportedModule) -> Arc<Vec<Symbol>> {
    let cache = MODULE_SYMBOLS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(entry) = cache.get(&module.path)
        && Arc::ptr_eq(&entry.program, &module.program)
    {
        return entry.symbols.clone();
    }
    if cache.len() >= MAX_CACHED_MODULES {
        cache.clear();
    }
    let symbols = Arc::new(declaration_symbols(&module.program, usize::MAX));
    cache.insert(
        module.path.clone(),
        CachedModuleSymbols {
            program: module.program.clone(),
            symbols: symbols.clone(),
        },
    );
    symbols
}

/// Index des `.`-Tokens unmittelbar vor dem Präfix (Byte-Offset `prefix_offset`).
fn member_dot(tokens: &[Token], prefix_offset: usize) -> Option<usize> {
    let mut before = tokens.partition_point(|token| token.span.end.offset <= prefix_offset);
    // Am Textende liegt das EOF-Token hinter dem Punkt.
    while before > 0 && tokens[before - 1].kind == TokenKind::EndOfFile {
        before -= 1;
    }
    let dot = before.checked_sub(1)?;
    matches!(&tokens[dot].kind, TokenKind::Symbol(symbol) if symbol == ".").then_some(dot)
}

/// Der Parser verwirft Deklarationen mit nicht geschlossenem Rumpf. Während des Tippens fehlen
/// die schließenden Klammern fast immer; sie werden hier ergänzt, damit lokale Symbole der
/// gerade bearbeiteten Funktion erhalten bleiben. Offsets vor dem Textende ändern sich nicht.
fn close_open_blocks(mut text: String) -> String {
    let tokens = analyze_active_document(&text);
    let open = tokens
        .tokens()
        .iter()
        .fold(0i32, |depth, token| match &token.kind {
            TokenKind::Symbol(symbol) if symbol == "{" => depth + 1,
            TokenKind::Symbol(symbol) if symbol == "}" => depth - 1,
            _ => depth,
        });
    for _ in 0..open.clamp(0, 64) {
        text.push_str("\n}");
    }
    text
}

fn import_context(chars: &[char], cursor: usize) -> bool {
    let start = chars[..cursor.min(chars.len())]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |index| index + 1);
    let line: String = chars[start..cursor.min(chars.len())].iter().collect();
    let line = line.trim_start();
    line.starts_with("using ") || line.starts_with("import ")
}

/// Existing string-only API retained for callers that do not need IntelliSense metadata.
pub fn suggest(prefix: &str, chars: &[char], cursor: usize) -> Vec<String> {
    suggest_detailed(prefix, chars, cursor, None, &[])
        .0
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

fn suggest_detailed(
    prefix: &str,
    chars: &[char],
    cursor: usize,
    active_path: Option<&Path>,
    project_files: &[PathBuf],
) -> (Vec<(String, CompletionDetail)>, Option<SignatureHelp>) {
    timed(Phase::Request, || {
        suggest_in_document(prefix, chars, cursor, active_path, project_files)
    })
}

fn suggest_in_document(
    prefix: &str,
    chars: &[char],
    cursor: usize,
    active_path: Option<&Path>,
    project_files: &[PathBuf],
) -> (Vec<(String, CompletionDetail)>, Option<SignatureHelp>) {
    if !prefix.is_empty() && prefix.chars().count() < MIN_PREFIX_CHARS {
        return (Vec::new(), None);
    }
    let cursor = cursor.min(chars.len());
    let prefix_start = cursor.saturating_sub(prefix.chars().count());
    let in_import = import_context(chars, cursor);
    let offset = byte_offset(chars, cursor);
    let text: String = chars.iter().collect();
    let document = analyze_active_document(&close_open_blocks(text));
    if document
        .comments()
        .iter()
        .any(|span| span.start.offset <= offset && offset <= span.end.offset)
        || document.tokens().iter().any(|token| {
            matches!(
                &token.kind,
                TokenKind::StringLiteral | TokenKind::CharacterLiteral
            ) && token.span.start.offset <= offset
                && offset < token.span.end.offset
        })
    {
        return (Vec::new(), None);
    }
    let program = document.program();
    let imports = active_path
        .map(|path| resolve_imports(path, program))
        .unwrap_or_default();
    let (mut candidates, signature_help) = timed(Phase::SymbolsAndTypes, || {
        // Die aktive Datei steht zuletzt und überschreibt gleichnamige importierte Deklarationen.
        let env = TypeEnv::new(
            imports
                .iter()
                .map(|module| module.program.as_ref())
                .chain([program]),
        );
        let class_here = enclosing_class(program, offset);
        let inference = Inference {
            env: &env,
            class: class_here,
        };
        let mut symbols = local_symbols(program, offset, &inference);
        for symbol in declaration_symbols(program, offset) {
            push_symbol(&mut symbols, symbol);
        }
        // Bei doppelten Namen in Importen gewinnt – wie in der Typumgebung – das spätere Modul.
        let import_symbols: Vec<Arc<Vec<Symbol>>> =
            imports.iter().rev().map(module_symbols).collect();
        let signature_help = signature_help(document.tokens(), offset, &env, &inference, &symbols);
        let dot = member_dot(document.tokens(), byte_offset(chars, prefix_start));
        let receiver_class = dot.and_then(|dot| {
            let lookup = |name: &str| local_type(&symbols, name);
            let scope = Scope {
                locals: &lookup,
                class: class_here,
            };
            let receiver = env.receiver_type(document.tokens(), dot, &scope)?;
            env.class_of_type(&receiver)
        });
        let mut candidates: Vec<(String, CompletionDetail, i32)> = Vec::new();
        let lower = prefix.to_lowercase();

        if dot.is_none() && !in_import {
            for keyword in KEYWORDS {
                if keyword.to_lowercase().starts_with(&lower) && *keyword != prefix {
                    candidates.push((
                        (*keyword).to_owned(),
                        CompletionDetail {
                            kind: CompletionKind::Keyword,
                            detail: "NORTEX-Schlüsselwort".into(),
                        },
                        80,
                    ));
                }
            }
        }

        if dot.is_none() || receiver_class.is_some() {
            if let Some(class) = receiver_class {
                for member in env.members(class, class_here) {
                    let (name, detail) = match member {
                        Member::Field(field) => (
                            field.name.as_str(),
                            CompletionDetail {
                                kind: CompletionKind::Variable,
                                detail: format!("Feld: {}", type_text(&field.type_ref)),
                            },
                        ),
                        Member::Method(method) => (
                            method.name.as_str(),
                            CompletionDetail {
                                kind: CompletionKind::Method,
                                detail: signature(method),
                            },
                        ),
                        Member::Variant(name) => (
                            name,
                            CompletionDetail {
                                kind: CompletionKind::Constant,
                                detail: format!("{}-Variante", class.name),
                            },
                        ),
                    };
                    add_candidate(&mut candidates, name, detail, &lower, prefix, 0);
                }
            } else {
                let mut used = HashSet::new();
                for symbol in &symbols {
                    if !used.insert(symbol.name.clone()) || symbol.name == prefix {
                        continue;
                    }
                    let distance =
                        offset.saturating_sub(symbol.span.start.offset).min(10_000) as i32;
                    let scope_bonus = if symbol.span.start.offset <= offset {
                        0
                    } else {
                        20
                    };
                    add_candidate(
                        &mut candidates,
                        &symbol.name,
                        CompletionDetail {
                            kind: symbol.kind,
                            detail: symbol.detail.clone(),
                        },
                        &lower,
                        prefix,
                        20 + scope_bonus + distance / 100,
                    );
                }
                if !in_import {
                    for symbol in import_symbols.iter().flat_map(|symbols| symbols.iter()) {
                        if !used.insert(symbol.name.clone()) || symbol.name == prefix {
                            continue;
                        }
                        add_candidate(
                            &mut candidates,
                            &symbol.name,
                            CompletionDetail {
                                kind: symbol.kind,
                                detail: symbol.detail.clone(),
                            },
                            &lower,
                            prefix,
                            35,
                        );
                    }
                }
                if in_import {
                    for using in &program.usings {
                        for name in &using.path {
                            add_candidate(
                                &mut candidates,
                                name,
                                CompletionDetail {
                                    kind: CompletionKind::Module,
                                    detail: "Importiertes Modul".into(),
                                },
                                &lower,
                                prefix,
                                10,
                            );
                        }
                    }
                    for part in imports
                        .iter()
                        .filter_map(|module| module.program.namespace.as_ref())
                        .flatten()
                    {
                        add_candidate(
                            &mut candidates,
                            part,
                            CompletionDetail {
                                kind: CompletionKind::Module,
                                detail: "Namespace importierter Module".into(),
                            },
                            &lower,
                            prefix,
                            12,
                        );
                    }
                    for path in project_files {
                        if !matches!(
                            path.extension().and_then(|extension| extension.to_str()),
                            Some("nx" | "nortex")
                        ) {
                            continue;
                        }
                        let Some(module) = path.file_stem().and_then(|name| name.to_str()) else {
                            continue;
                        };
                        add_candidate(
                            &mut candidates,
                            module,
                            CompletionDetail {
                                kind: CompletionKind::Module,
                                detail: "Projektquelldatei".into(),
                            },
                            &lower,
                            prefix,
                            10,
                        );
                    }
                }
                for (name, count) in identifier_occurrences(document.tokens(), prefix, offset) {
                    if used.insert(name.clone()) {
                        add_candidate(
                            &mut candidates,
                            &name,
                            CompletionDetail {
                                kind: CompletionKind::Variable,
                                detail: "Bezeichner in Datei".into(),
                            },
                            &lower,
                            prefix,
                            40 - count.min(10) as i32,
                        );
                    }
                }
            }
        }

        (candidates, signature_help)
    });

    candidates.sort_by(|left, right| {
        left.2
            .cmp(&right.2)
            .then_with(|| left.0.to_lowercase().cmp(&right.0.to_lowercase()))
            .then_with(|| left.0.cmp(&right.0))
    });
    candidates.dedup_by(|left, right| left.0 == right.0);
    candidates.truncate(MAX_SUGGESTIONS);
    let items = candidates
        .into_iter()
        .map(|(name, detail, _)| (name, detail))
        .collect();
    (items, signature_help)
}

fn add_candidate(
    candidates: &mut Vec<(String, CompletionDetail, i32)>,
    name: &str,
    detail: CompletionDetail,
    lower: &str,
    prefix: &str,
    base_score: i32,
) {
    if name != prefix && name.to_lowercase().starts_with(lower) {
        let case_penalty = i32::from(!name.starts_with(prefix));
        candidates.push((name.to_owned(), detail, base_score + case_penalty));
    }
}

fn identifier_occurrences(
    tokens: &[nortex_compiler::lexer::Token],
    prefix: &str,
    offset: usize,
) -> Vec<(String, usize)> {
    let mut counts = HashMap::<String, usize>::new();
    for token in tokens {
        if token.kind == TokenKind::Identifier
            && token.span.start.offset != offset
            && token
                .lexeme
                .to_lowercase()
                .starts_with(&prefix.to_lowercase())
        {
            *counts.entry(token.lexeme.clone()).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub items: Vec<String>,
    pub details: Vec<CompletionDetail>,
    pub signature_help: Option<SignatureHelp>,
    pub selected: usize,
    pub prefix: String,
    /// Cursor (Zeichenindex), an dem das Popup geöffnet wurde.
    pub anchor: usize,
}

impl Completion {
    pub fn open(text: &str, cursor: usize) -> Option<Self> {
        Self::open_for_file(text, cursor, None, &[])
    }

    /// Ohne Pfad (ungespeicherte Datei) gibt es keinen Import-Graph und damit keine
    /// importierten Symbole.
    pub fn open_for_file(
        text: &str,
        cursor: usize,
        active_path: Option<&Path>,
        project_files: &[PathBuf],
    ) -> Option<Self> {
        let chars: Vec<char> = text.chars().collect();
        let prefix = prefix_at(&chars, cursor).unwrap_or_default();
        let (suggestions, signature_help) =
            suggest_detailed(&prefix, &chars, cursor, active_path, project_files);
        if signature_help.is_none() && (prefix.is_empty() || suggestions.is_empty()) {
            return None;
        }
        let (items, details): (Vec<_>, Vec<_>) = if prefix.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            suggestions.into_iter().unzip()
        };
        Some(Self {
            items,
            details,
            signature_help,
            selected: 0,
            prefix,
            anchor: cursor,
        })
    }

    pub fn manual(text: &str, cursor: usize) -> Option<Self> {
        Self::manual_for_file(text, cursor, None, &[])
    }

    pub fn manual_for_file(
        text: &str,
        cursor: usize,
        active_path: Option<&Path>,
        project_files: &[PathBuf],
    ) -> Option<Self> {
        let chars: Vec<char> = text.chars().collect();
        let prefix = prefix_at(&chars, cursor).unwrap_or_default();
        let (suggestions, signature_help) =
            suggest_detailed(&prefix, &chars, cursor, active_path, project_files);
        if suggestions.is_empty() && signature_help.is_none() {
            return None;
        }
        let (items, details): (Vec<_>, Vec<_>) = suggestions.into_iter().unzip();
        Some(Self {
            items,
            details,
            signature_help,
            selected: 0,
            prefix,
            anchor: cursor,
        })
    }

    /// Umlaufende Auswahl (`-1` = hoch, `1` = runter).
    pub fn step(&mut self, delta: isize) {
        let length = self.items.len() as isize;
        if length > 0 {
            self.selected = (self.selected as isize + delta).rem_euclid(length) as usize;
        }
    }

    /// Ersetzt ausschließlich das eingegebene Präfix; Text nach dem Cursor bleibt erhalten.
    pub fn accept(&self, text: &str) -> Option<(String, usize)> {
        let item = self.items.get(self.selected)?;
        let chars: Vec<char> = text.chars().collect();
        let start = self.anchor.checked_sub(self.prefix.chars().count())?;
        if self.anchor > chars.len() {
            return None;
        }
        let mut result: String = chars[..start].iter().collect();
        result.push_str(item);
        result.extend(&chars[self.anchor..]);
        Some((result, start + item.chars().count()))
    }
}

fn signature_help(
    tokens: &[Token],
    cursor_offset: usize,
    env: &TypeEnv,
    inference: &Inference,
    locals: &[Symbol],
) -> Option<SignatureHelp> {
    let mut calls: Vec<(usize, usize)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.span.start.offset >= cursor_offset {
            break;
        }
        if !matches!(&token.kind, TokenKind::Symbol(_)) {
            continue;
        }
        match token.lexeme.as_str() {
            "(" => calls.push((index, 0)),
            ")" => {
                calls.pop();
            }
            "," => {
                if let Some((_, arguments)) = calls.last_mut() {
                    *arguments += 1;
                }
            }
            _ => {}
        }
    }
    let (open, argument) = calls.pop()?;
    let callee = tokens.get(open.checked_sub(1)?)?;
    if callee.kind != TokenKind::Identifier {
        return None;
    }
    let callee_name = callee.lexeme.as_str();
    let lookup = |name: &str| local_type(locals, name);
    let scope = Scope {
        locals: &lookup,
        class: inference.class,
    };
    let preceding_dot = open
        .checked_sub(2)
        .filter(|index| matches!(&tokens[*index].kind, TokenKind::Symbol(symbol) if symbol == "."));
    let function = match preceding_dot {
        Some(dot) => {
            let owner = env.receiver_type(tokens, dot, &scope)?;
            env.members(env.class_of_type(&owner)?, inference.class)
                .into_iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == callee_name => Some(method),
                    _ => None,
                })?
        }
        None => inference
            .class
            .and_then(|name| env.class(name))
            .and_then(|class| {
                env.members(class, inference.class)
                    .into_iter()
                    .find_map(|member| match member {
                        Member::Method(method) if method.name == callee_name => Some(method),
                        _ => None,
                    })
            })
            .or_else(|| env.function(callee_name))?,
    };
    let active_parameter = function
        .parameters
        .get(argument)
        .map(|parameter| (argument + 1, parameter.name.clone()));
    Some(SignatureHelp {
        signature: signature(function),
        active_parameter,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn prefix_is_unicode_safe_and_waits_for_two_characters() {
        assert_eq!(prefix_at(&chars("w"), 1), None);
        assert_eq!(prefix_at(&chars("1a"), 2), None);
        assert_eq!(prefix_at(&chars("ä = wh"), 6).as_deref(), Some("wh"));
    }

    #[test]
    fn suggestions_are_capped_deduplicated_and_rank_document_symbols() {
        let text = "int counter; int count2; int counted; int country;";
        let items = suggest("co", &chars(text), 0);
        assert!(items.len() <= MAX_SUGGESTIONS);
        assert!(items.contains(&"const".to_string()));
        assert!(items.contains(&"counter".to_string()));
        assert!(!suggest("int", &chars(text), 0).contains(&"int".to_string()));
    }

    #[test]
    fn functions_and_types_include_intellisense_details() {
        let source =
            "public int calculate(int amount, string label) { return amount; }\nclass Worker {}";
        let completion = Completion::manual(source, source.chars().count()).unwrap();
        let function = completion.items.iter().position(|name| name == "calculate");
        assert!(function.is_some());
        let type_index = completion.items.iter().position(|name| name == "Worker");
        assert!(type_index.is_some());
        assert!(
            completion
                .details
                .iter()
                .any(|detail| detail.detail == "calculate(amount: int, label: string) -> int")
        );
    }

    #[test]
    fn member_completion_uses_receiver_type() {
        let source = "class Worker { public int run() { return 1; } }\nWorker worker;\nworker.ru";
        let completion = Completion::open(source, source.chars().count()).unwrap();
        assert_eq!(completion.items, vec!["run"]);
        assert_eq!(completion.details[0].kind, CompletionKind::Method);
    }

    /// Legt ein Projekt im Temp-Verzeichnis an und liefert dessen Ordner.
    fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nx_ide_completion_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (file, content) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        dir
    }

    /// Öffnet die Vervollständigung am Ende von `source`, als wäre es der Puffer von `file`.
    fn complete_in(dir: &Path, file: &str, source: &str) -> Option<Completion> {
        let path = dir.join(file);
        fs::write(&path, source).unwrap();
        Completion::open_for_file(source, source.chars().count(), Some(&path), &[])
    }

    fn items_in(dir: &Path, file: &str, source: &str) -> Vec<String> {
        complete_in(dir, file, source)
            .map(|completion| completion.items)
            .unwrap_or_default()
    }

    /// Wie `Strg+Leertaste`: öffnet auch ohne eingetipptes Präfix.
    fn manual_in(dir: &Path, file: &str, source: &str) -> Option<Completion> {
        let path = dir.join(file);
        fs::write(&path, source).unwrap();
        Completion::manual_for_file(source, source.chars().count(), Some(&path), &[])
    }

    const SHOP_LIB: &str = "public class Item { public string title; private int secret; public int price() { return 1; } }\npublic Item make(int count) { return new Item(); }\nItem shared;\n";

    #[test]
    fn imported_function_signature_return_type_and_members_form_one_chain() {
        let dir = project("chain", &[("lib.nx", SHOP_LIB)]);

        let call = "using lib;\npublic void run() { var item = make(";
        let help = complete_in(&dir, "main.nx", call)
            .and_then(|completion| completion.signature_help)
            .unwrap();
        assert_eq!(help.signature, "make(count: int) -> Item");
        assert_eq!(help.active_parameter, Some((1, "count".into())));

        let suggestion = complete_in(&dir, "main.nx", "using lib;\nmak").unwrap();
        assert_eq!(suggestion.items, vec!["make"]);
        assert_eq!(suggestion.details[0].kind, CompletionKind::Function);
        assert_eq!(suggestion.details[0].detail, "make(count: int) -> Item");

        let members = "using lib;\npublic void run() { var item = make(1); item.";
        let all = manual_in(&dir, "main.nx", members).unwrap();
        assert_eq!(all.items, vec!["price", "title"]);
        assert!(!all.items.contains(&"secret".to_owned()));
        assert_eq!(
            items_in(&dir, "main.nx", &format!("{members}ti")),
            vec!["title"]
        );
        assert_eq!(
            items_in(
                &dir,
                "main.nx",
                "using lib;\npublic void run() { make(1).ti"
            ),
            vec!["title"]
        );
        assert_eq!(
            items_in(
                &dir,
                "main.nx",
                "using lib;\npublic void run() { make(1).price().x"
            ),
            Vec::<String>::new()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_variables_types_and_methods_are_suggested_with_their_kind() {
        let dir = project("kinds", &[("lib.nx", SHOP_LIB)]);
        let source = "using lib;\nsha";
        let shared = complete_in(&dir, "main.nx", source).unwrap();
        assert_eq!(shared.items, vec!["shared"]);
        assert_eq!(shared.details[0].kind, CompletionKind::Variable);
        let item = complete_in(&dir, "main.nx", "using lib;\nIte").unwrap();
        assert_eq!(item.items, vec!["Item"]);
        assert_eq!(item.details[0].kind, CompletionKind::Class);
        assert_eq!(
            items_in(&dir, "main.nx", "using lib;\nshared.pr"),
            vec!["price"]
        );
        assert_eq!(
            items_in(
                &dir,
                "main.nx",
                "using lib;\npublic Item twin() { return shared; }\ntwin().ti"
            ),
            vec!["title"]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn symbols_are_only_visible_through_using() {
        let dir = project(
            "visibility",
            &[
                ("lib.nx", SHOP_LIB),
                ("other.nx", "public int other_value() { return 1; }"),
            ],
        );
        assert!(items_in(&dir, "main.nx", "mak").is_empty());
        assert!(items_in(&dir, "main.nx", "using other;\nmak").is_empty());
        assert_eq!(items_in(&dir, "main.nx", "using lib;\nmak"), vec!["make"]);
        assert!(items_in(&dir, "main.nx", "using lib;\nother_v").is_empty());
        assert_eq!(
            items_in(&dir, "main.nx", "using lib;\nusing other;\nother_v"),
            vec!["other_value"]
        );
        // ohne Dateipfad (ungespeicherter Puffer) gibt es keinen Import-Graph
        let source = "using lib;\nmak";
        assert!(Completion::open_for_file(source, source.chars().count(), None, &[]).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn transitive_imports_are_visible_exactly_like_in_the_compiler() {
        let dir = project(
            "transitive",
            &[
                ("a.nx", "using b;\npublic int from_a() { return deep(); }\n"),
                ("b.nx", "public int deep() { return 1; }\n"),
            ],
        );
        assert_eq!(items_in(&dir, "main.nx", "using a;\ndee"), vec!["deep"]);
        let main = dir.join("main.nx");
        fs::write(
            &main,
            "using a;\npublic int main() { return deep() + from_a(); }\n",
        )
        .unwrap();
        let result = nortex_compiler::driver::check_file(&main).unwrap();
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|item| item.message.contains("deep")),
            "{:?}",
            result.diagnostics
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn relative_nested_and_root_imports_resolve() {
        let dir = project(
            "nested",
            &[
                (
                    "sub/helper.nx",
                    "using sub.inner;\npublic int helper_fn() { return 1; }\n",
                ),
                ("sub/inner.nx", "public int inner_fn() { return 2; }\n"),
                ("shared/mod.nx", "public int shared_fn() { return 3; }\n"),
            ],
        );
        let source = "using sub.helper;\nusing shared;\n";
        assert_eq!(
            items_in(&dir, "main.nx", &format!("{source}helper_")),
            vec!["helper_fn"]
        );
        assert_eq!(
            items_in(&dir, "main.nx", &format!("{source}inner_")),
            vec!["inner_fn"]
        );
        assert_eq!(
            items_in(&dir, "main.nx", &format!("{source}shared_")),
            vec!["shared_fn"]
        );
        // `sub/helper.nx` importiert `sub.inner` relativ zur Projektwurzel
        let from_sub = dir.join("sub/page.nx");
        let page = "using helper;\nhelper_";
        fs::write(&from_sub, page).unwrap();
        let completion =
            Completion::open_for_file(page, page.chars().count(), Some(&from_sub), &[]).unwrap();
        assert_eq!(completion.items, vec!["helper_fn"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn namespaces_of_resolved_imports_are_offered_in_using_lines() {
        let dir = project(
            "namespaces",
            &[(
                "lib.nx",
                "namespace Shop.Tools;\npublic int tool() { return 1; }\n",
            )],
        );
        let completion = complete_in(&dir, "main.nx", "using lib;\nusing Sh").unwrap();
        assert_eq!(completion.items, vec!["Shop"]);
        assert_eq!(completion.details[0].kind, CompletionKind::Module);
        // der Namespace erlaubt keinen qualifizierten Zugriff; der Name selbst bleibt sichtbar
        assert_eq!(items_in(&dir, "main.nx", "using lib;\ntoo"), vec!["tool"]);
        assert!(items_in(&dir, "main.nx", "using lib;\nShop.to").is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_base_classes_provide_only_non_private_members() {
        let dir = project("inherit", &[("lib.nx", SHOP_LIB)]);
        let source = "using lib;\npublic class Special : Item { public int own() { return 1; } public void f() { this.";
        let all = manual_in(&dir, "main.nx", source).unwrap();
        assert!(all.items.contains(&"price".to_owned()));
        assert!(all.items.contains(&"title".to_owned()));
        assert!(all.items.contains(&"own".to_owned()));
        assert!(!all.items.contains(&"secret".to_owned()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn name_conflicts_stay_deduplicated_and_ambiguous_types_offer_no_members() {
        let dir = project(
            "conflict",
            &[
                ("first.nx", "public int make() { return 1; }\n"),
                ("second.nx", "public string make() { return \"\"; }\n"),
            ],
        );
        let both = "using first;\nusing second;\n";
        let names = items_in(&dir, "main.nx", &format!("{both}mak"));
        assert_eq!(names, vec!["make"]);
        // Datei-eigene Deklaration hat Vorrang vor importierter
        let own = "using first;\npublic string make() { return \"\"; }\nmak";
        let completion = complete_in(&dir, "main.nx", own).unwrap();
        assert_eq!(completion.details[0].detail, "make() -> string");
        // mehrdeutiger Rückgabetyp: keine erfundenen Member
        let call = format!("{both}public void f() {{ var v = make(); v.");
        assert!(manual_in(&dir, "main.nx", &call).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cyclic_imports_terminate_and_still_provide_symbols() {
        let dir = project(
            "cycle",
            &[
                ("a.nx", "using b;\npublic int in_a() { return 1; }\n"),
                ("b.nx", "using a;\npublic int in_b() { return 2; }\n"),
            ],
        );
        assert_eq!(items_in(&dir, "a.nx", "using b;\nin_"), vec!["in_b"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn changed_signatures_deleted_symbols_and_removed_imports_update_results() {
        let dir = project("changes", &[("lib.nx", SHOP_LIB)]);
        let call = "using lib;\npublic void run() { make(";
        assert_eq!(
            complete_in(&dir, "main.nx", call)
                .unwrap()
                .signature_help
                .unwrap()
                .signature,
            "make(count: int) -> Item"
        );
        fs::write(
            dir.join("lib.nx"),
            "public class Item { public string title; }\npublic Item make(int count, string label) { return new Item(); }\n",
        )
        .unwrap();
        assert_eq!(
            complete_in(&dir, "main.nx", call)
                .unwrap()
                .signature_help
                .unwrap()
                .signature,
            "make(count: int, label: string) -> Item"
        );
        assert_eq!(
            items_in(
                &dir,
                "main.nx",
                "using lib;\npublic void run() { var item = make(1, \"\"); item.pr"
            ),
            Vec::<String>::new()
        );

        fs::write(dir.join("lib.nx"), "public int unrelated() { return 0; }\n").unwrap();
        assert!(items_in(&dir, "main.nx", "using lib;\nmak").is_empty());
        assert_eq!(
            items_in(&dir, "main.nx", "using lib;\nunrel"),
            vec!["unrelated"]
        );
        assert!(items_in(&dir, "main.nx", "unrel").is_empty());

        fs::remove_file(dir.join("lib.nx")).unwrap();
        assert!(items_in(&dir, "main.nx", "using lib;\nunrel").is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn module_symbols_are_cached_until_the_module_changes() {
        let dir = project("cache", &[("lib.nx", SHOP_LIB)]);
        let main = dir.join("main.nx");
        fs::write(&main, "using lib;\n").unwrap();
        let program = parse_program("using lib;\n");
        let first = resolve_imports(&main, &program);
        let second = resolve_imports(&main, &program);
        assert!(Arc::ptr_eq(
            &module_symbols(&first[0]),
            &module_symbols(&second[0])
        ));
        fs::write(
            dir.join("lib.nx"),
            format!("{SHOP_LIB}public int added() {{ return 1; }}\n"),
        )
        .unwrap();
        let third = resolve_imports(&main, &program);
        let updated = module_symbols(&third[0]);
        assert!(updated.iter().any(|symbol| symbol.name == "added"));
        fs::remove_dir_all(dir).unwrap();
    }

    fn parse_program(source: &str) -> Program {
        nortex_compiler::parser::Parser::new(
            nortex_compiler::lexer::Lexer::new(source).tokenize().tokens,
        )
        .parse()
        .program
    }

    #[test]
    fn incomplete_code_keeps_general_and_imported_completion_working() {
        let dir = project("broken", &[("lib.nx", SHOP_LIB)]);
        let broken = "using lib;\npublic int f( {\n var = ;\n}\npublic void g() { var item = make(1); item.pr";
        assert_eq!(items_in(&dir, "main.nx", broken), vec!["price"]);
        assert!(
            items_in(&dir, "main.nx", "using lib;\npublic void g() { whi")
                .contains(&"while".to_owned())
        );
        let _ = complete_in(&dir, "main.nx", "using ;;; ( ) . . class {");
        let _ = complete_in(&dir, "main.nx", "using lib;\nmake(1).make(2).");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parameters_and_locals_of_methods_are_completed() {
        let source = "class Calc { public int run(int amount) { var doubled = amount; return amo";
        let completion = Completion::open(source, source.chars().count()).unwrap();
        assert!(completion.items.contains(&"amount".to_owned()));
        let source = "class Calc { public int run(int amount) { var doubled = amount; dou";
        assert_eq!(
            Completion::open(source, source.chars().count()).map(|completion| completion.items),
            Some(vec!["doubled".to_owned()])
        );
    }

    #[test]
    fn member_completion_without_a_known_type_offers_nothing() {
        let source = "public void f() { var thing = unknown(); thing.";
        assert!(Completion::open(source, source.chars().count()).is_none());
        let manual = Completion::manual(source, source.chars().count());
        assert!(manual.is_none());
    }

    #[test]
    fn inferred_factory_return_type_drives_member_completion() {
        let source = "class Worker { public int run() { return 1; } }\npublic Worker make() { return new Worker(); }\npublic void use() { var worker = make(); worker.ru }";
        let cursor = source.find("worker.ru").unwrap() + "worker.ru".len();
        let completion = Completion::open(source, cursor).unwrap();
        assert_eq!(completion.items, vec!["run"]);
    }

    #[test]
    fn import_context_offers_project_source_modules() {
        let files = vec![
            PathBuf::from("src/network.nx"),
            PathBuf::from("src/storage.nortex"),
            PathBuf::from("README.md"),
        ];
        let source = "using net";
        let completion =
            Completion::open_for_file(source, source.chars().count(), None, &files).unwrap();
        assert!(completion.items.contains(&"network".to_owned()));
        assert!(!completion.items.contains(&"README".to_owned()));
        assert!(
            completion
                .details
                .iter()
                .any(|detail| detail.kind == CompletionKind::Module)
        );
    }

    #[test]
    fn call_context_shows_the_active_parameter_signature() {
        let source =
            "public int calculate(int amount, string label) { return amount; }\ncalculate(1, ";
        let completion = Completion::manual(source, source.chars().count()).unwrap();
        let help = completion.signature_help.unwrap();
        assert_eq!(
            help.signature,
            "calculate(amount: int, label: string) -> int"
        );
        assert_eq!(help.active_parameter, Some((2, "label".into())));
    }

    #[test]
    fn accept_keeps_text_after_cursor_and_handles_unicode() {
        let text = "ä = whi + 1";
        let popup = Completion::open(text, 7).unwrap();
        let (new_text, cursor) = popup.accept(text).unwrap();
        assert_eq!(new_text, "ä = while + 1");
        assert_eq!(cursor, 9);
    }

    #[test]
    fn malformed_source_does_not_panic() {
        let source = "public int f( { let x = ; x";
        let _ = Completion::manual(source, source.chars().count());
    }

    #[test]
    fn completion_is_suppressed_in_comments_and_literals() {
        let comment = "// while";
        assert!(Completion::open(comment, comment.chars().count()).is_none());
        let literal = "let text = \"while\";";
        assert!(Completion::open(literal, 15).is_none());
    }

    #[test]
    fn method_calls_show_signature_help() {
        let source = "class Worker { public int run(int count) { return count; } }\nWorker worker;\nworker.run(";
        let completion = Completion::open(source, source.chars().count()).unwrap();
        assert_eq!(
            completion
                .signature_help
                .as_ref()
                .map(|help| help.signature.as_str()),
            Some("run(count: int) -> int")
        );
        assert!(completion.items.is_empty());
    }
}

//! Source-to-AST front-end driver.

use std::path::{Path, PathBuf};

use crate::ast::Program;
use crate::diagnostics::Diagnostic;
use crate::hir;
use crate::lexer::Lexer;
use crate::mir::{self, MirBody};
use crate::optimizer;
use crate::parser::Parser;
use crate::semantic;

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
    pub token_count: usize,
}

#[derive(Debug, Clone)]
pub struct LoweredMirFunction {
    pub name: String,
    pub body: MirBody,
    /// Externe C-ABI-Funktion ohne Rumpf; `body` enthält nur die Signatur.
    pub external: bool,
}

#[derive(Debug, Clone)]
pub struct MirCompileResult {
    pub functions: Vec<LoweredMirFunction>,
    pub diagnostics: Vec<Diagnostic>,
    pub token_count: usize,
}

impl MirCompileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == crate::diagnostics::Severity::Error)
    }
}

impl CheckResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == crate::diagnostics::Severity::Error)
    }
}

pub fn compile_source_to_mir(source: &str) -> MirCompileResult {
    let checked = check_source(source);
    lower_checked_program(checked)
}

pub fn compile_file_to_mir(path: impl AsRef<Path>) -> std::io::Result<MirCompileResult> {
    check_file(path).map(lower_checked_program)
}

fn lower_checked_program(checked: CheckResult) -> MirCompileResult {
    let CheckResult {
        program,
        mut diagnostics,
        token_count,
    } = checked;
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == crate::diagnostics::Severity::Error)
    {
        return MirCompileResult {
            functions: Vec::new(),
            diagnostics,
            token_count,
        };
    }

    let hir_program = match hir::lower_program(&program) {
        Ok(program) => program,
        Err(lowering_diagnostics) => {
            diagnostics.extend(lowering_diagnostics);
            return MirCompileResult {
                functions: Vec::new(),
                diagnostics,
                token_count,
            };
        }
    };
    if hir_program.functions.is_empty() {
        diagnostics.push(Diagnostic::error_with_code(
            "NX2001",
            "Die Quelle enthält keine Funktionskörper, die in MIR übersetzt werden können.",
            Default::default(),
        ));
        return MirCompileResult {
            functions: Vec::new(),
            diagnostics,
            token_count,
        };
    }

    let globals = match mir::lowering::lower_globals(&hir_program.globals) {
        Ok(globals) => globals,
        Err(lowering_diagnostics) => {
            diagnostics.extend(lowering_diagnostics);
            return MirCompileResult {
                functions: Vec::new(),
                diagnostics,
                token_count,
            };
        }
    };

    let mut functions = Vec::with_capacity(hir_program.functions.len() + hir_program.externs.len());
    for external in &hir_program.externs {
        let mut body = external_signature_body(external);
        body.globals = globals.clone();
        functions.push(LoweredMirFunction {
            name: external.name.clone(),
            body,
            external: true,
        });
    }
    for function in &hir_program.functions {
        match mir::lowering::lower_function(function, &globals, &hir_program.classes) {
            Ok(mut body) => {
                if let Err(error) = body.validate() {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX2003",
                        format!("Ungültiges MIR vor Optimierung: {error}."),
                        function.span,
                    ));
                    continue;
                }
                optimizer::optimize(&mut body);
                if let Err(error) = body.validate() {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX2003",
                        format!("Ungültiges MIR nach Optimierung: {error}."),
                        function.span,
                    ));
                    continue;
                }
                functions.push(LoweredMirFunction {
                    name: function.name.clone(),
                    body,
                    external: false,
                });
            }
            Err(lowering_diagnostics) => diagnostics.extend(lowering_diagnostics),
        }
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == crate::diagnostics::Severity::Error)
    {
        functions.clear();
    }
    MirCompileResult {
        functions,
        diagnostics,
        token_count,
    }
}

fn external_signature_body(external: &hir::HirExtern) -> MirBody {
    let mut body = MirBody::new(external.parameter_types.len());
    let local = |type_name: &String| mir::LocalDecl {
        name: None,
        type_name: type_name.clone(),
        field_types: Vec::new(),
        is_mut: false,
        is_user_variable: false,
        global_symbol: None,
    };
    body.local_decls.push(local(&external.return_type));
    body.local_decls
        .extend(external.parameter_types.iter().map(local));
    body
}

pub fn check_source(source: &str) -> CheckResult {
    check_source_with_options(source, false)
}

fn check_source_with_options(source: &str, allow_intrinsics: bool) -> CheckResult {
    let lexed = Lexer::new(source).tokenize();
    let token_count = lexed.tokens.len().saturating_sub(1);
    let parsed = Parser::new(lexed.tokens).parse();
    let mut diagnostics = lexed.diagnostics;
    diagnostics.extend(parsed.diagnostics);
    diagnostics.extend(semantic::analyze_with_options(
        &parsed.program,
        semantic::SemanticOptions { allow_intrinsics },
    ));
    CheckResult {
        program: parsed.program,
        diagnostics,
        token_count,
    }
}

pub fn check_file(path: impl AsRef<Path>) -> std::io::Result<CheckResult> {
    check_file_with_source(path, None)
}

/// `check_file` mit optionalem Ersatzinhalt für die Einstiegsdatei (ungespeicherter
/// Editorpuffer); importierte Module werden weiterhin von der Platte gelesen.
pub fn check_file_with_source(
    path: impl AsRef<Path>,
    source: Option<&str>,
) -> std::io::Result<CheckResult> {
    let path = path.as_ref();
    let allow_intrinsics = is_compiler_owned_standard_library_source(path);
    let loaded = semantic::resolver::ModuleResolver::load_with_source(path, source)?;
    let token_count = loaded.token_count;
    let mut diagnostics = loaded.diagnostics;
    diagnostics.extend(semantic::analyze_with_options(
        &loaded.program,
        semantic::SemanticOptions { allow_intrinsics },
    ));
    // Parser-Diagnostics tragen bereits ihre Datei; der Rest gehört zur Einstiegsdatei.
    let display_path = path.display().to_string();
    for diagnostic in &mut diagnostics {
        if diagnostic.file.is_none() {
            diagnostic.file = Some(display_path.clone());
        }
    }
    Ok(CheckResult {
        program: loaded.program,
        diagnostics,
        token_count,
    })
}

/// Only sources physically located inside the compiler's configured standard
/// library root may declare compiler intrinsics. A directory merely named
/// `standard-library` elsewhere in a user's project is not trusted.
fn is_compiler_owned_standard_library_source(path: &Path) -> bool {
    let candidates: Vec<PathBuf> = match crate::linker::dev_workspace_root() {
        Some(root) => vec![root.join("standard-library")],
        None => std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|dir| {
                vec![
                    dir.join("standard-library"),
                    dir.join("../standard-library"),
                ]
            })
            .unwrap_or_default(),
    };
    let Ok(source_path) = std::fs::canonicalize(path) else {
        return false;
    };
    candidates
        .into_iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .any(|root| source_path.starts_with(root))
}

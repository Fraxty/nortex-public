//! Initial bidirectional expression type checker for the NORTEX frontend.
//!
//! This pass intentionally implements the well-defined non-generic expression
//! subset first. Generic substitution, CFG-based flow analysis, ownership and
//! borrow checking remain separate compiler stages.

use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostics::{Diagnostic, SourceSpan};

#[derive(Clone, Default)]
struct FunctionType {
    parameters: Vec<String>,
    result: String,
    generic_parameters: Vec<String>,
    declaring_type: Option<String>,
    is_static: bool,
    is_private: bool,
}

#[derive(Default, Clone)]
struct ClassType {
    fields: HashMap<String, String>,
    private_fields: std::collections::HashSet<String>,
    methods: HashMap<String, Vec<FunctionType>>,
    constructors: Vec<FunctionType>,
    enum_variants: std::collections::HashSet<String>,
    base: Option<String>,
    /// All declared base types (base class and interfaces) for assignability.
    supertypes: Vec<String>,
    /// Declared generic parameters, used for substitution at use sites.
    generics: Vec<String>,
}

#[derive(Clone)]
struct LocalBinding {
    type_name: String,
    id: Option<usize>,
}

type Scope = HashMap<String, LocalBinding>;

struct CallArguments<'a> {
    generic_arguments: &'a [TypeReference],
    expressions: &'a [Expression],
    types: &'a [Option<String>],
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BorrowProjection {
    Field(String),
    Index(Option<String>),
    Dereference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BorrowPlace {
    root: Option<String>,
    projection: Vec<BorrowProjection>,
}

struct Checker<'a> {
    functions: HashMap<String, Vec<FunctionType>>,
    classes: HashMap<String, ClassType>,
    global_variables: HashMap<String, String>,
    known_types: std::collections::HashSet<String>,
    /// Names declared as generic parameters anywhere in the program. Until
    /// full generic inference exists, an unsubstituted parameter is treated
    /// as compatible to avoid false positives.
    type_parameters: std::collections::HashSet<String>,
    moved_values: std::collections::HashSet<usize>,
    next_binding_id: usize,
    diagnostics: &'a mut Vec<Diagnostic>,
}

pub(super) fn check(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let mut checker = Checker {
        functions: HashMap::new(),
        classes: HashMap::new(),
        global_variables: HashMap::new(),
        known_types: std::collections::HashSet::new(),
        type_parameters: std::collections::HashSet::new(),
        moved_values: std::collections::HashSet::new(),
        next_binding_id: 0,
        diagnostics,
    };
    checker.collect_signatures(program);
    checker.check_program(program);
}

fn field_scope(fields: &HashMap<String, String>) -> Scope {
    fields
        .iter()
        .map(|(name, type_name)| {
            (
                name.clone(),
                LocalBinding {
                    type_name: type_name.clone(),
                    id: None,
                },
            )
        })
        .collect()
}

mod calls;
mod checking;
mod expressions;
mod type_utils;

use type_utils::*;

//! First semantic validation pass for NORTEX.
//!
//! The pass currently validates declarations, type names, generic arity,
//! naming conventions and the explicitly permitted operator overload set.
//! Full type inference and borrow checking are later compiler stages.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::{Diagnostic, SourceSpan};

mod declarations;
mod expressions;
mod functions;
pub mod resolver;
mod statements;
mod type_checker;

use declarations::*;
use expressions::*;
use functions::*;
use statements::*;

#[derive(Debug, Clone)]
struct KnownType {
    min_generic_arity: usize,
    max_generic_arity: usize,
}

impl KnownType {
    fn exact(generic_arity: usize) -> Self {
        Self {
            min_generic_arity: generic_arity,
            max_generic_arity: generic_arity,
        }
    }

    fn range(minimum: usize, maximum: usize) -> Self {
        Self {
            min_generic_arity: minimum,
            max_generic_arity: maximum,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SemanticOptions {
    /// Intrinsic declarations are reserved for compiler-owned standard-library
    /// source files. The capability is intentionally crate-private.
    pub allow_intrinsics: bool,
}

pub fn analyze(program: &Program) -> Vec<Diagnostic> {
    analyze_with_options(program, SemanticOptions::default())
}

pub(crate) fn analyze_with_options(program: &Program, options: SemanticOptions) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    validate_using_imports(program, &mut diagnostics);
    let mut known_types: HashMap<String, KnownType> = HashMap::new();
    let mut declared_kinds: HashMap<String, TypeDeclarationKind> = HashMap::new();
    let mut top_level_type_names = HashSet::new();
    let mut top_level_function_signatures = HashSet::new();
    let global_names: HashSet<String> = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::GlobalVariable(global) => Some(global.name.clone()),
            _ => None,
        })
        .collect();
    let immutable_global_values: HashSet<String> = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::GlobalVariable(global)
                if !global.modifiers.iter().any(|modifier| modifier == "mut") =>
            {
                Some(global.name.clone())
            }
            _ => None,
        })
        .collect();

    if let Some(namespace) = &program.namespace {
        for part in namespace {
            if !part
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
            {
                diagnostics.push(Diagnostic::warning(
                    format!("Namespace-Segment '{part}' sollte in PascalCase geschrieben werden."),
                    program
                        .declarations
                        .first()
                        .map(declaration_span)
                        .unwrap_or_default(),
                ));
            }
        }
    }

    // Primitive names come from the shared canonical type catalogue. These
    // are NORTEX-owned types and do not imply any dependency on .NET.
    for name in crate::type_system::BUILTIN_TYPE_NAMES {
        known_types.insert((*name).to_string(), KnownType::exact(0));
    }
    // NORTEX also defines object and dynamic explicitly; neither is the
    // automatic base type of every value.
    for name in ["object", "dynamic", "IntList", "StringList"] {
        known_types.insert(name.to_string(), KnownType::exact(0));
    }
    // Standard-library and runtime generic declarations. List and Vec are
    // distinct nominal types, not aliases and not interchangeable.
    for (name, arity) in [
        ("Option", 1),
        ("Result", 2),
        ("Task", 1),
        ("Future", 1),
        ("Box", 1),
        ("Rc", 1),
        ("Arc", 1),
        ("Cell", 1),
        ("RefCell", 1),
        ("Mutex", 1),
        ("RwLock", 1),
        ("Span", 1),
        ("Memory", 1),
    ] {
        known_types.insert(name.to_string(), KnownType::exact(arity));
    }
    // Collection types are declared in System.Collections and become visible
    // only when the program imports that module. The library source files can
    // therefore declare their own public collection types without colliding
    // with compiler-predeclared names.
    if program.usings.iter().any(|item| {
        let namespace = item.path.join(".");
        namespace == "System.Collections" || namespace.starts_with("System.Collections.")
    }) {
        for (name, arity) in [
            ("List", 1),
            ("Vec", 1),
            ("Dictionary", 2),
            ("HashSet", 1),
            ("Queue", 1),
            ("Stack", 1),
        ] {
            known_types.insert(name.to_string(), KnownType::exact(arity));
        }
    }
    // Function/delegate types have a variable number of generic arguments.
    // Func<T1, ..., TResult> requires at least one argument; Action may be empty.
    known_types.insert("Func".into(), KnownType::range(1, 17));
    known_types.insert("Action".into(), KnownType::range(0, 16));
    known_types.insert("Exception".into(), KnownType::exact(0));
    known_types.insert("RuntimeException".into(), KnownType::exact(0));

    for declaration in &program.declarations {
        if let Declaration::Class(class) = declaration {
            if !top_level_type_names.insert(class.name.clone()) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1001",
                    format!(
                        "Doppelte Typdeklaration '{}'. Bezeichner sind case-sensitive.",
                        class.name
                    ),
                    class.span,
                ));
                continue;
            }
            if known_types.contains_key(&class.name) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1001",
                    format!("Der Typname '{}' ist bereits durch NORTEX vordefiniert und kann nicht erneut deklariert werden.", class.name), class.span));
            } else {
                known_types.insert(
                    class.name.clone(),
                    KnownType::exact(class.generic_parameters.len()),
                );
                declared_kinds.insert(class.name.clone(), class.kind);
            }
            validate_type_name(&class.name, class.kind, class.span, &mut diagnostics);
        }
    }

    let class_declarations: HashMap<&str, &ClassDeclaration> = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Class(class) => Some((class.name.as_str(), class)),
            Declaration::Function(_) | Declaration::GlobalVariable(_) => None,
        })
        .collect();
    let mut declared_global_names = HashSet::new();
    let function_names: HashSet<&str> = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Function(function) => Some(function.name.as_str()),
            _ => None,
        })
        .collect();
    for declaration in &program.declarations {
        match declaration {
            Declaration::GlobalVariable(global) => {
                if !declared_global_names.insert(global.name.clone())
                    || function_names.contains(global.name.as_str())
                    || top_level_type_names.contains(&global.name)
                    || known_types.contains_key(&global.name)
                {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        format!(
                            "Doppelter oder bereits belegter Top-Level-Bezeichner '{}'.",
                            global.name
                        ),
                        global.span,
                    ));
                }
                validate_global_variable(global, &known_types, &mut diagnostics);
            }
            Declaration::Class(class) => analyze_class(
                class,
                &known_types,
                &declared_kinds,
                &class_declarations,
                options.allow_intrinsics,
                GlobalValueContext {
                    names: &global_names,
                    immutable: &immutable_global_values,
                },
                &mut diagnostics,
            ),
            Declaration::Function(function) => {
                let signature = function_signature(function);
                if !top_level_function_signatures.insert(signature) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        format!("Doppelte Funktionssignatur '{}'.", function.name),
                        function.span,
                    ));
                }
                validate_free_function_declaration(
                    function,
                    options.allow_intrinsics,
                    &mut diagnostics,
                );
                validate_function(
                    function,
                    None,
                    FunctionValidationContext {
                        known_types: &known_types,
                        enclosing_generics: &HashSet::new(),
                        fields: &HashSet::new(),
                        immutable_fields: &HashSet::new(),
                        global_values: &global_names,
                        immutable_globals: &immutable_global_values,
                        class_declarations: &class_declarations,
                    },
                    &mut diagnostics,
                );
            }
        }
    }

    let mut global_values: HashSet<String> = [
        "Console",
        "Text",
        "Convert",
        "File",
        "Directory",
        "IntList",
        "StringList",
        "System",
        "Math",
        "Time",
        "Debug",
        "Some",
        "None",
        "Ok",
        "Err",
        "print",
        "println",
        "read",
        "readText",
        "writeText",
        "cleanup",
        "assert",
        "panic",
        "spawn",
        "block_on",
        "true",
        "false",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    for name in known_types.keys() {
        global_values.insert(name.clone());
    }
    global_values.extend(global_names.iter().cloned());
    for declaration in &program.declarations {
        if let Declaration::Function(function) = declaration {
            global_values.insert(function.name.clone());
        }
    }
    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                let mut scope = HashSet::new();
                scope.insert("this".to_string());
                for parameter in &function.parameters {
                    scope.insert(parameter.name.clone());
                }
                if let Some(body) = &function.body {
                    validate_block_names(body, &scope, &global_values, &mut diagnostics);
                }
            }
            Declaration::Class(class) => {
                let mut class_values: HashSet<String> =
                    ["this", "base"].into_iter().map(str::to_string).collect();
                class_values.extend(global_names.iter().cloned());
                for member in &class.members {
                    match member {
                        MemberDeclaration::Field(field) => {
                            class_values.insert(field.name.clone());
                        }
                        MemberDeclaration::Method(method) => {
                            class_values.insert(method.name.clone());
                        }
                        _ => {}
                    }
                }
                // Non-private members of base classes and interfaces are in
                // scope by simple name. Bounded walk tolerates cyclic bases.
                let mut pending: Vec<&str> = class
                    .base_types
                    .iter()
                    .map(|base| base.name.as_str())
                    .collect();
                let mut visited = HashSet::new();
                while let Some(base_name) = pending.pop() {
                    if base_name == class.name || !visited.insert(base_name) {
                        continue;
                    }
                    let Some(base_class) = class_declarations.get(base_name) else {
                        continue;
                    };
                    for member in &base_class.members {
                        let (name, modifiers) = match member {
                            MemberDeclaration::Field(field) => (&field.name, &field.modifiers),
                            MemberDeclaration::Method(method) => (&method.name, &method.modifiers),
                            _ => continue,
                        };
                        if !modifiers.iter().any(|modifier| modifier == "private") {
                            class_values.insert(name.clone());
                        }
                    }
                    pending.extend(base_class.base_types.iter().map(|base| base.name.as_str()));
                }
                for member in &class.members {
                    if let MemberDeclaration::Method(method) = member {
                        let mut scope = class_values.clone();
                        for parameter in &method.parameters {
                            scope.insert(parameter.name.clone());
                        }
                        if let Some(body) = &method.body {
                            validate_block_names(body, &scope, &global_values, &mut diagnostics);
                        }
                    }
                }
            }
            Declaration::GlobalVariable(_) => {}
        }
    }

    validate_abstract_contracts(program, &mut diagnostics);
    type_checker::check(program, &mut diagnostics);
    diagnostics
}

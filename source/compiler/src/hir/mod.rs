//! Lowered, function-oriented representation of the supported source subset.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;

use crate::ast::{
    BlockStatement, ClassDeclaration, Declaration, Expression, FunctionDeclaration,
    MemberDeclaration, Program, ReferenceKind, Statement, TypeDeclarationKind, TypeReference,
};
use crate::builtins;
use crate::diagnostics::{Diagnostic, SourceSpan};

#[derive(Debug, Clone)]
pub struct HirProgram {
    pub functions: Vec<HirFunction>,
    pub externs: Vec<HirExtern>,
    pub globals: Vec<HirGlobal>,
    /// Classes the native backend can lay out and construct.
    pub classes: Vec<HirClass>,
}

/// A natively lowered class layout with pointer-sized fields.
/// Methods are ordinary functions named `Class.method` that take `this` first;
/// the constructor is `Class.new` and runs the field initializers.
#[derive(Debug, Clone)]
pub struct HirClass {
    pub name: String,
    pub fields: Vec<HirField>,
    pub base_class: Option<String>,
    pub class_id: usize,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct HirField {
    pub name: String,
    pub type_name: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct HirGlobal {
    pub name: String,
    pub type_name: String,
    pub mutable: bool,
    pub public: bool,
    pub initializer: HirExpression,
    pub span: SourceSpan,
}

/// Externe C-ABI-Funktion (`extern "C"`), die erst der Linker auflöst.
#[derive(Debug, Clone)]
pub struct HirExtern {
    pub name: String,
    pub return_type: String,
    pub parameter_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<HirParameter>,
    pub body: HirBlock,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct HirParameter {
    pub name: String,
    pub type_name: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub statements: Vec<HirStatement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub enum HirStatement {
    Let {
        name: String,
        type_name: String,
        mutable: bool,
        initializer: Option<HirExpression>,
        span: SourceSpan,
    },
    Return(Option<HirExpression>, SourceSpan),
    If {
        condition: HirExpression,
        then_branch: Box<HirStatement>,
        else_branch: Option<Box<HirStatement>>,
        span: SourceSpan,
    },
    While {
        condition: HirExpression,
        body: Box<HirStatement>,
        span: SourceSpan,
    },
    For {
        initializer: Option<Box<HirStatement>>,
        condition: Option<HirExpression>,
        increment: Option<HirExpression>,
        body: Box<HirStatement>,
        span: SourceSpan,
    },
    LoopControl {
        is_continue: bool,
        span: SourceSpan,
    },
    Expression(HirExpression, SourceSpan),
    Block(HirBlock),
}

#[derive(Debug, Clone)]
pub enum HirExpression {
    Identifier(String, SourceSpan),
    Integer(String, SourceSpan),
    Float(String, SourceSpan),
    String(String, SourceSpan),
    Boolean(bool, SourceSpan),
    Call {
        callee: String,
        arguments: Vec<HirExpression>,
        return_type: String,
        span: SourceSpan,
    },
    Unary {
        operator: String,
        operand: Box<HirExpression>,
        span: SourceSpan,
    },
    Conditional {
        condition: Box<HirExpression>,
        when_true: Box<HirExpression>,
        when_false: Box<HirExpression>,
        span: SourceSpan,
    },
    Assignment {
        target: String,
        through_reference: bool,
        operator: String,
        value: Box<HirExpression>,
        span: SourceSpan,
    },
    Binary {
        left: Box<HirExpression>,
        operator: String,
        right: Box<HirExpression>,
        span: SourceSpan,
    },
    /// Instance field read; `object` is always a local or `this`.
    Field {
        object: Box<HirExpression>,
        class: String,
        field: String,
        type_name: String,
        span: SourceSpan,
    },
    FieldAssignment {
        object: Box<HirExpression>,
        class: String,
        field: String,
        type_name: String,
        operator: String,
        value: Box<HirExpression>,
        span: SourceSpan,
    },
    /// Heap object construction with a statically resolved class constructor.
    New {
        class: String,
        arguments: Vec<HirExpression>,
        span: SourceSpan,
    },
}

impl HirExpression {
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Identifier(_, span)
            | Self::Integer(_, span)
            | Self::Float(_, span)
            | Self::String(_, span)
            | Self::Boolean(_, span) => *span,
            Self::Call { span, .. }
            | Self::Unary { span, .. }
            | Self::Conditional { span, .. }
            | Self::Assignment { span, .. }
            | Self::Binary { span, .. }
            | Self::Field { span, .. }
            | Self::FieldAssignment { span, .. }
            | Self::New { span, .. } => *span,
        }
    }
}

pub fn lower_program(program: &Program) -> Result<HirProgram, Vec<Diagnostic>> {
    let mut functions = Vec::new();
    let mut externs = Vec::new();
    let mut globals = Vec::new();
    let mut classes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut env = LowerEnv::default();
    let specialized_classes = specialize_generic_classes(program);
    let mut lowered_classes: Vec<ClassDeclaration> = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Class(class)
                if class.kind == TypeDeclarationKind::Class
                    && class.generic_parameters.is_empty() =>
            {
                Some(class.clone())
            }
            _ => None,
        })
        .collect();
    lowered_classes.extend(specialized_classes);
    specialize_generic_methods(program, &mut lowered_classes);
    let class_names: std::collections::HashSet<_> = lowered_classes
        .iter()
        .map(|class| class.name.as_str())
        .collect();

    let mut pending_classes = Vec::new();
    let mut next_class_id = 1;
    for declaration in &program.declarations {
        if let Declaration::Function(function) = declaration {
            env.signatures
                .entry(function.name.clone())
                .or_default()
                .push(FunctionSignature {
                    parameter_count: function.parameters.len(),
                    return_type: type_name(&function.return_type),
                    is_generic: !function.generic_parameters.is_empty(),
                });
        }
    }
    for class in &lowered_classes {
        pending_classes.push((class, next_class_id));
        next_class_id += 1;
    }
    let mut unresolved = pending_classes;
    while !unresolved.is_empty() {
        let before = unresolved.len();
        let mut next = Vec::new();
        for (class, class_id) in unresolved {
            let base_name = class.base_types.first().map(type_name);
            if let Some(base_name) = &base_name
                && let Some(reason) = env.unsupported_classes.get(base_name)
            {
                env.unsupported_classes.insert(
                    class.name.clone(),
                    format!("Basisklasse '{base_name}' ist nicht unterstützt: {reason}"),
                );
                continue;
            }
            if let Some(base_name) = &base_name
                && !env.classes.contains_key(base_name)
                && class_names.contains(base_name.as_str())
                && !env.unsupported_classes.contains_key(base_name)
            {
                next.push((class, class_id));
                continue;
            }
            match native_class_blocker(class, &class_names) {
                None => {
                    let base = base_name
                        .as_ref()
                        .and_then(|base| env.classes.get(base))
                        .cloned();
                    if let Some(reason) = validate_inheritance(class, base.as_ref()) {
                        env.unsupported_classes.insert(class.name.clone(), reason);
                    } else {
                        let info = class_info(class, base.as_ref(), class_id);
                        env.classes.insert(class.name.clone(), info);
                    }
                }
                Some(reason) => {
                    env.unsupported_classes.insert(class.name.clone(), reason);
                }
            }
        }
        if next.len() == before {
            for (class, _) in next {
                env.unsupported_classes.insert(
                    class.name.clone(),
                    "Basisklasse unbekannt, nicht unterstützt oder zyklisch".to_owned(),
                );
            }
            break;
        }
        unresolved = next;
    }
    let env = env;

    for declaration in &program.declarations {
        if let Declaration::GlobalVariable(global) = declaration {
            let Some(initializer) = &global.initializer else {
                diagnostics.push(unsupported(
                    "Globale Variablen benötigen einen Initialisierungswert.",
                    global.span,
                ));
                continue;
            };
            if let Some(initializer) = lower_expression(initializer, &env, &mut diagnostics) {
                globals.push(HirGlobal {
                    name: global.name.clone(),
                    type_name: type_name(&global.type_ref),
                    mutable: global.modifiers.iter().any(|modifier| modifier == "mut"),
                    public: global.modifiers.iter().any(|modifier| modifier == "public"),
                    initializer,
                    span: global.span,
                });
            }
        }
    }

    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                if function.body.is_none()
                    && function
                        .modifiers
                        .iter()
                        .any(|modifier| modifier == "extern")
                {
                    externs.push(HirExtern {
                        name: function.name.clone(),
                        return_type: type_name(&function.return_type),
                        parameter_types: function
                            .parameters
                            .iter()
                            .map(|parameter| type_name(&parameter.type_ref))
                            .collect(),
                    });
                    continue;
                }
                if let Some(lowered) = lower_callable(
                    function,
                    &function.name,
                    &env,
                    &mut diagnostics,
                    None,
                    Vec::new(),
                ) {
                    functions.push(lowered);
                }
            }
            Declaration::Class(_) => {}
            Declaration::GlobalVariable(_) => {}
        }
    }
    for class in &lowered_classes {
        if env.classes.contains_key(&class.name) {
            lower_native_class(class, &env, &mut functions, &mut classes, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(HirProgram {
            functions,
            externs,
            globals,
            classes,
        })
    } else {
        Err(diagnostics)
    }
}

#[derive(Clone)]
struct FunctionSignature {
    parameter_count: usize,
    return_type: String,
    is_generic: bool,
}

#[derive(Clone)]
struct MethodInfo {
    is_static: bool,
    is_virtual: bool,
    return_type: String,
    parameter_count: usize,
    parameter_types: Vec<String>,
    function_name: String,
}

#[derive(Clone)]
struct ClassInfo {
    fields: Vec<(String, String)>,
    methods: HashMap<String, MethodInfo>,
    constructor_arity: usize,
    base_class: Option<String>,
    class_id: usize,
}

#[derive(Clone)]
struct MethodContext {
    class: String,
    instance: bool,
}

/// Lowering environment: free-function signatures (reachable through `Deref`),
/// the native class table and the lexical scopes of the function being lowered.
#[derive(Default)]
struct LowerEnv {
    signatures: HashMap<String, Vec<FunctionSignature>>,
    classes: HashMap<String, ClassInfo>,
    unsupported_classes: HashMap<String, String>,
    current: RefCell<Option<MethodContext>>,
    scopes: RefCell<Vec<HashMap<String, String>>>,
}

impl Deref for LowerEnv {
    type Target = HashMap<String, Vec<FunctionSignature>>;

    fn deref(&self) -> &Self::Target {
        &self.signatures
    }
}

impl LowerEnv {
    fn enter(&self, context: Option<(&str, bool)>, scope: HashMap<String, String>) {
        *self.current.borrow_mut() = context.map(|(class, instance)| MethodContext {
            class: class.to_owned(),
            instance,
        });
        *self.scopes.borrow_mut() = vec![scope];
    }

    fn leave(&self) {
        *self.current.borrow_mut() = None;
        self.scopes.borrow_mut().clear();
    }

    fn scope_depth(&self) -> usize {
        self.scopes.borrow().len()
    }

    fn push_scope(&self) {
        self.scopes.borrow_mut().push(HashMap::new());
    }

    fn truncate_scopes(&self, depth: usize) {
        self.scopes.borrow_mut().truncate(depth);
    }

    fn declare(&self, name: &str, type_name: String) {
        if let Some(scope) = self.scopes.borrow_mut().last_mut() {
            scope.insert(name.to_owned(), type_name);
        }
    }

    fn lookup(&self, name: &str) -> Option<String> {
        self.scopes
            .borrow()
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn current_context(&self) -> Option<MethodContext> {
        self.current.borrow().clone()
    }

    /// A bare field name inside an instance method of a native class.
    fn implicit_field(&self, name: &str) -> Option<(String, String)> {
        if self.lookup(name).is_some() {
            return None;
        }
        let context = self.current_context().filter(|context| context.instance)?;
        let info = self.classes.get(&context.class)?;
        info.fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, type_name)| (context.class, type_name.clone()))
    }

    fn has_method_in_current_class(&self, name: &str) -> bool {
        self.current_context()
            .and_then(|context| self.classes.get(&context.class))
            .is_some_and(|info| info.methods.contains_key(name))
    }

    fn reject_unsupported_class(
        &self,
        class: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(reason) = self.unsupported_classes.get(class) else {
            return false;
        };
        diagnostics.push(unsupported(
            format!("Klasse '{class}' wird vom nativen Backend nicht unterstützt: {reason}."),
            span,
        ));
        true
    }

    /// Class of a receiver, which must be a local, parameter or `this`.
    fn object_class(
        &self,
        object: &HirExpression,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<String> {
        let HirExpression::Identifier(name, _) = object else {
            diagnostics.push(unsupported(
                "Member-Zugriffe sind nur auf lokalen Objekten, Parametern oder 'this' unterstützt.",
                span,
            ));
            return None;
        };
        let class = self.lookup(name).unwrap_or_default();
        if self.reject_unsupported_class(&class, span, diagnostics) {
            return None;
        }
        if !self.classes.contains_key(&class) {
            diagnostics.push(unsupported(
                format!("'{name}' ist kein Objekt einer nativ unterstützten Klasse."),
                span,
            ));
            return None;
        }
        Some(class)
    }
}

fn is_native_field_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "bool"
            | "byte"
            | "sbyte"
            | "short"
            | "ushort"
            | "int"
            | "uint"
            | "long"
            | "ulong"
            | "nint"
            | "nuint"
            | "isize"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "double"
    )
}

/// Returns why a class cannot be compiled natively, or `None` when it can.
fn native_class_blocker(
    class: &ClassDeclaration,
    class_names: &std::collections::HashSet<&str>,
) -> Option<String> {
    if !class.generic_parameters.is_empty() {
        return Some("generische Klassen benötigen Monomorphisierung".to_owned());
    }
    if class.base_types.len() > 1 {
        return Some("Mehrfachvererbung und Interfaces sind nicht unterstützt".to_owned());
    }
    if let Some(base) = class.base_types.first()
        && (!base.generic_arguments.is_empty()
            || base.array_depth > 0
            || !class_names.contains(base.name.as_str())
            || base.name == class.name)
    {
        return Some(format!(
            "Basisklasse '{}' ist nicht unterstützt",
            type_name(base)
        ));
    }
    if class
        .modifiers
        .iter()
        .any(|modifier| modifier == "abstract")
    {
        return Some("abstrakte Klassen benötigen dynamischen Dispatch".to_owned());
    }
    let mut method_names = std::collections::HashSet::new();
    let mut constructors = 0;
    for member in &class.members {
        match member {
            MemberDeclaration::Field(field) => {
                if field
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "static" || modifier == "const")
                {
                    return Some(format!(
                        "statische und const-Felder ('{}') sind nicht abgebildet",
                        field.name
                    ));
                }
                let field_type = type_name(&field.type_ref);
                if !is_native_field_type(&field_type)
                    && field_type != "string"
                    && !class_names.contains(field_type.as_str())
                {
                    return Some(format!(
                        "Feld '{}' hat den nativen Backend unbekannten Typ '{field_type}'",
                        field.name
                    ));
                }
            }
            MemberDeclaration::Method(method) => {
                if method.body.is_none() {
                    return Some(format!("Methode '{}' besitzt keinen Körper", method.name));
                }
                if method
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "virtual" || modifier == "override")
                    && method.modifiers.iter().any(|modifier| modifier == "static")
                {
                    return Some(format!(
                        "Virtuelle Methode '{}' darf nicht static sein",
                        method.name
                    ));
                }
                if method.name == class.name {
                    constructors += 1;
                    if constructors > 1 {
                        return Some("mehrere Konstruktoren sind nicht unterstützt".to_owned());
                    }
                } else if !method_names.insert(method.name.clone()) {
                    return Some(format!("Methode '{}' ist überladen", method.name));
                }
            }
            MemberDeclaration::Operator(_) => {
                return Some("Operatorüberladungen sind nicht unterstützt".to_owned());
            }
            MemberDeclaration::Variant(_) => {
                return Some("Enum-Varianten sind nicht unterstützt".to_owned());
            }
        }
    }
    None
}

fn class_info(class: &ClassDeclaration, base: Option<&ClassInfo>, class_id: usize) -> ClassInfo {
    let mut info = ClassInfo {
        fields: base.map_or_else(Vec::new, |base| base.fields.clone()),
        methods: base.map_or_else(HashMap::new, |base| base.methods.clone()),
        constructor_arity: 0,
        base_class: class.base_types.first().map(|base| base.name.clone()),
        class_id,
    };
    for member in &class.members {
        match member {
            MemberDeclaration::Field(field) => info
                .fields
                .push((field.name.clone(), type_name(&field.type_ref))),
            MemberDeclaration::Method(method) if method.name == class.name => {
                info.constructor_arity = method.parameters.len();
            }
            MemberDeclaration::Method(method) => {
                info.methods.insert(
                    method.name.clone(),
                    MethodInfo {
                        is_static: method.modifiers.iter().any(|modifier| modifier == "static"),
                        is_virtual: method
                            .modifiers
                            .iter()
                            .any(|modifier| modifier == "virtual" || modifier == "override")
                            || base
                                .and_then(|base| base.methods.get(&method.name))
                                .is_some_and(|method| method.is_virtual),
                        return_type: type_name(&method.return_type),
                        parameter_count: method.parameters.len(),
                        parameter_types: method
                            .parameters
                            .iter()
                            .map(|parameter| type_name(&parameter.type_ref))
                            .collect(),
                        function_name: format!("{}.{}", class.name, method.name),
                    },
                );
            }
            _ => {}
        }
    }
    info
}

fn validate_inheritance(class: &ClassDeclaration, base: Option<&ClassInfo>) -> Option<String> {
    let Some(base) = base else {
        if !class.base_types.is_empty() {
            return Some("Basisklasse konnte nicht aufgelöst werden".to_owned());
        }
        return None;
    };
    if base.constructor_arity != 0 {
        return Some("Basisklassen benötigen derzeit einen parameterlosen Konstruktor".to_owned());
    }
    for member in &class.members {
        match member {
            MemberDeclaration::Field(field)
                if base.fields.iter().any(|(name, _)| name == &field.name) =>
            {
                return Some(format!(
                    "Feld '{}' überschattet ein geerbtes Feld",
                    field.name
                ));
            }
            MemberDeclaration::Method(method) if method.name != class.name => {
                let inherited = base.methods.get(&method.name);
                let is_override = method
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "override");
                if is_override && !inherited.is_some_and(|method| method.is_virtual) {
                    return Some(format!(
                        "Methode '{}' überschreibt keine virtuelle Basismethode",
                        method.name
                    ));
                }
                if inherited.is_some() && !is_override {
                    return Some(format!(
                        "Geerbte Methode '{}' muss mit 'override' überschrieben werden",
                        method.name
                    ));
                }
                if let Some(inherited) = inherited {
                    if inherited.is_static
                        || method.modifiers.iter().any(|modifier| modifier == "static")
                        || inherited.return_type != type_name(&method.return_type)
                        || inherited.parameter_count != method.parameters.len()
                        || inherited.parameter_types
                            != method
                                .parameters
                                .iter()
                                .map(|parameter| type_name(&parameter.type_ref))
                                .collect::<Vec<_>>()
                    {
                        return Some(format!(
                            "Override '{}' hat eine inkompatible Signatur",
                            method.name
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn lower_native_class(
    class: &ClassDeclaration,
    env: &LowerEnv,
    functions: &mut Vec<HirFunction>,
    classes: &mut Vec<HirClass>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    classes.push(HirClass {
        name: class.name.clone(),
        fields: env.classes[&class.name]
            .fields
            .iter()
            .map(|(name, ty)| HirField {
                name: name.clone(),
                type_name: ty.clone(),
                span: class.span,
            })
            .collect(),
        base_class: env.classes[&class.name].base_class.clone(),
        class_id: env.classes[&class.name].class_id,
        span: class.span,
    });

    let mut prologue = Vec::new();
    if let Some(base) = &env.classes[&class.name].base_class {
        prologue.push(HirStatement::Expression(
            HirExpression::Call {
                callee: format!("{base}.new"),
                arguments: vec![HirExpression::Identifier("this".to_owned(), class.span)],
                return_type: "void".to_owned(),
                span: class.span,
            },
            class.span,
        ));
    }
    prologue.extend(lower_field_initializers(class, env, diagnostics));
    let constructor_name = format!("{}.new", class.name);
    let mut has_constructor = false;
    for member in &class.members {
        let MemberDeclaration::Method(method) = member else {
            continue;
        };
        let lowered = if method.name == class.name {
            has_constructor = true;
            lower_callable(
                method,
                &constructor_name,
                env,
                diagnostics,
                Some((&class.name, true)),
                prologue.clone(),
            )
        } else {
            let is_instance = !method.modifiers.iter().any(|modifier| modifier == "static");
            lower_callable(
                method,
                &format!("{}.{}", class.name, method.name),
                env,
                diagnostics,
                Some((&class.name, is_instance)),
                Vec::new(),
            )
        };
        functions.extend(lowered);
    }
    if !has_constructor {
        functions.push(HirFunction {
            name: constructor_name,
            return_type: "void".to_owned(),
            parameters: vec![HirParameter {
                name: "this".to_owned(),
                type_name: class.name.clone(),
                span: class.span,
            }],
            body: HirBlock {
                statements: prologue,
                span: class.span,
            },
            span: class.span,
        });
    }
}

/// Field initializers become `this.field = value` statements at the start of the constructor.
fn lower_field_initializers(
    class: &ClassDeclaration,
    env: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<HirStatement> {
    let mut scope = HashMap::new();
    scope.insert("this".to_owned(), class.name.clone());
    env.enter(Some((&class.name, true)), scope);
    let mut statements = Vec::new();
    for member in &class.members {
        let MemberDeclaration::Field(field) = member else {
            continue;
        };
        let Some(initializer) = &field.initializer else {
            continue;
        };
        if let Some(value) = lower_expression(initializer, env, diagnostics) {
            statements.push(HirStatement::Expression(
                HirExpression::FieldAssignment {
                    object: Box::new(HirExpression::Identifier("this".to_owned(), field.span)),
                    class: class.name.clone(),
                    field: field.name.clone(),
                    type_name: type_name(&field.type_ref),
                    operator: "=".to_owned(),
                    value: Box::new(value),
                    span: field.span,
                },
                field.span,
            ));
        }
    }
    env.leave();
    statements
}

/// `context` is `(class, has_this)` for methods of native classes.
fn lower_callable(
    function: &FunctionDeclaration,
    name: &str,
    env: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
    context: Option<(&str, bool)>,
    mut prologue: Vec<HirStatement>,
) -> Option<HirFunction> {
    let Some(body) = &function.body else {
        return None;
    };
    let mut parameters = Vec::new();
    let mut scope = HashMap::new();
    if let Some((class, true)) = context {
        parameters.push(HirParameter {
            name: "this".to_owned(),
            type_name: class.to_owned(),
            span: function.span,
        });
        scope.insert("this".to_owned(), class.to_owned());
    }
    for parameter in &function.parameters {
        let parameter_type = type_name(&parameter.type_ref);
        scope.insert(parameter.name.clone(), parameter_type.clone());
        parameters.push(HirParameter {
            name: parameter.name.clone(),
            type_name: parameter_type,
            span: parameter.span,
        });
    }
    env.enter(context, scope);
    let body = lower_block(body, env, diagnostics);
    env.leave();
    let mut body = body?;
    if !prologue.is_empty() {
        prologue.append(&mut body.statements);
        body.statements = prologue;
    }
    Some(HirFunction {
        name: name.to_owned(),
        return_type: type_name(&function.return_type),
        parameters,
        body,
        span: function.span,
    })
}

fn lower_block(
    block: &BlockStatement,
    signatures: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HirBlock> {
    let depth = signatures.scope_depth();
    signatures.push_scope();
    let mut statements = Vec::with_capacity(block.statements.len());
    let mut lowered_all = true;
    for statement in &block.statements {
        match lower_statement(statement, signatures, diagnostics) {
            Some(statement) => statements.push(statement),
            None => {
                lowered_all = false;
                break;
            }
        }
    }
    signatures.truncate_scopes(depth);
    lowered_all.then_some(HirBlock {
        statements,
        span: block.span,
    })
}

fn lower_statement(
    statement: &Statement,
    signatures: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HirStatement> {
    match statement {
        Statement::Block(block) => Some(HirStatement::Block(lower_block(
            block,
            signatures,
            diagnostics,
        )?)),
        Statement::Variable(variable) => {
            let Some(type_ref) = &variable.type_ref else {
                diagnostics.push(unsupported(
                    "Variablentyp-Inferenz wird in dieser Lowering-Stufe nicht unterstützt.",
                    variable.span,
                ));
                return None;
            };
            let variable_type = type_name(type_ref);
            if signatures.reject_unsupported_class(&variable_type, variable.span, diagnostics) {
                return None;
            }
            let initializer = match &variable.initializer {
                Some(expression) => Some(lower_expression(expression, signatures, diagnostics)?),
                None => None,
            };
            signatures.declare(&variable.name, variable_type.clone());
            Some(HirStatement::Let {
                name: variable.name.clone(),
                type_name: variable_type,
                mutable: matches!(variable.mutability, crate::ast::VariableMutability::Mutable),
                initializer,
                span: variable.span,
            })
        }
        Statement::Return(value, span) => {
            let value = match value {
                Some(expression) => Some(lower_expression(expression, signatures, diagnostics)?),
                None => None,
            };
            Some(HirStatement::Return(value, *span))
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Some(HirStatement::If {
            condition: lower_expression(condition, signatures, diagnostics)?,
            then_branch: Box::new(lower_statement(then_branch, signatures, diagnostics)?),
            else_branch: match else_branch {
                Some(branch) => Some(Box::new(lower_statement(branch, signatures, diagnostics)?)),
                None => None,
            },
            span: *span,
        }),
        Statement::While {
            condition,
            body,
            span,
        } => Some(HirStatement::While {
            condition: lower_expression(condition, signatures, diagnostics)?,
            body: Box::new(lower_statement(body, signatures, diagnostics)?),
            span: *span,
        }),
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            span,
        } => {
            let depth = signatures.scope_depth();
            signatures.push_scope();
            let lowered = Some(HirStatement::For {
                initializer: match initializer {
                    Some(statement) => Some(Box::new(lower_statement(
                        statement,
                        signatures,
                        diagnostics,
                    )?)),
                    None => None,
                },
                condition: match condition {
                    Some(expression) => {
                        Some(lower_expression(expression, signatures, diagnostics)?)
                    }
                    None => None,
                },
                increment: match increment {
                    Some(expression) => {
                        Some(lower_expression(expression, signatures, diagnostics)?)
                    }
                    None => None,
                },
                body: Box::new(lower_statement(body, signatures, diagnostics)?),
                span: *span,
            });
            signatures.truncate_scopes(depth);
            lowered
        }
        Statement::Break(span) => Some(HirStatement::LoopControl {
            is_continue: false,
            span: *span,
        }),
        Statement::Continue(span) => Some(HirStatement::LoopControl {
            is_continue: true,
            span: *span,
        }),
        Statement::Expression(expression, span) => Some(HirStatement::Expression(
            lower_expression(expression, signatures, diagnostics)?,
            *span,
        )),
        _ => {
            diagnostics.push(unsupported(
                "Diese Kontrollflussanweisung wird in der aktuellen Lowering-Stufe nicht unterstützt.",
                statement_span(statement),
            ));
            None
        }
    }
}

fn lower_expression(
    expression: &Expression,
    signatures: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HirExpression> {
    match expression {
        Expression::Identifier(name, span) => {
            if let Some((class, type_name)) = signatures.implicit_field(name) {
                return Some(HirExpression::Field {
                    object: Box::new(HirExpression::Identifier("this".to_owned(), *span)),
                    class,
                    field: name.clone(),
                    type_name,
                    span: *span,
                });
            }
            Some(HirExpression::Identifier(name.clone(), *span))
        }
        Expression::Member { object, name, span } => {
            let object = lower_expression(object, signatures, diagnostics)?;
            let (class, type_name) = resolve_field(&object, name, *span, signatures, diagnostics)?;
            Some(HirExpression::Field {
                object: Box::new(object),
                class,
                field: name.clone(),
                type_name,
                span: *span,
            })
        }
        Expression::ObjectCreation {
            type_ref,
            arguments,
            span,
        } => {
            let class = type_name(type_ref);
            if signatures.reject_unsupported_class(&class, *span, diagnostics) {
                return None;
            }
            let info = signatures.classes.get(&class).filter(|_| {
                type_ref.array_depth == 0 && type_ref.reference_kind == ReferenceKind::Value
            });
            let Some(info) = info else {
                diagnostics.push(unsupported(
                    format!(
                        "Objekterzeugung von '{class}' wird im nativen Backend nicht unterstützt."
                    ),
                    *span,
                ));
                return None;
            };
            if arguments.len() != info.constructor_arity {
                diagnostics.push(unsupported(
                    format!(
                        "Der Konstruktor von '{class}' erwartet {} Argumente, übergeben wurden {}.",
                        info.constructor_arity,
                        arguments.len()
                    ),
                    *span,
                ));
                return None;
            }
            Some(HirExpression::New {
                class,
                arguments: arguments
                    .iter()
                    .map(|argument| lower_expression(argument, signatures, diagnostics))
                    .collect::<Option<Vec<_>>>()?,
                span: *span,
            })
        }
        Expression::Integer(value, span) => Some(HirExpression::Integer(value.clone(), *span)),
        Expression::Float(value, span) => Some(HirExpression::Float(value.clone(), *span)),
        Expression::String(value, span) => Some(HirExpression::String(value.clone(), *span)),
        Expression::Boolean(value, span) => Some(HirExpression::Boolean(*value, *span)),
        Expression::Group(inner, _) => lower_expression(inner, signatures, diagnostics),
        Expression::Unary {
            operator,
            operand,
            span,
        } => Some(HirExpression::Unary {
            operator: operator.clone(),
            operand: Box::new(lower_expression(operand, signatures, diagnostics)?),
            span: *span,
        }),
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            span,
        } => Some(HirExpression::Conditional {
            condition: Box::new(lower_expression(condition, signatures, diagnostics)?),
            when_true: Box::new(lower_expression(when_true, signatures, diagnostics)?),
            when_false: Box::new(lower_expression(when_false, signatures, diagnostics)?),
            span: *span,
        }),
        Expression::Assignment {
            target,
            operator,
            value,
            span,
        } => {
            let (target, through_reference) = match target.as_ref() {
                Expression::Identifier(target, target_span) => {
                    if let Some((class, type_name)) = signatures.implicit_field(target) {
                        return lower_field_assignment(
                            HirExpression::Identifier("this".to_owned(), *target_span),
                            class,
                            target,
                            type_name,
                            operator,
                            value,
                            *span,
                            signatures,
                            diagnostics,
                        );
                    }
                    (target.clone(), false)
                }
                Expression::Member { object, name, .. } => {
                    let object = lower_expression(object, signatures, diagnostics)?;
                    let (class, type_name) =
                        resolve_field(&object, name, *span, signatures, diagnostics)?;
                    return lower_field_assignment(
                        object,
                        class,
                        name,
                        type_name,
                        operator,
                        value,
                        *span,
                        signatures,
                        diagnostics,
                    );
                }
                Expression::Unary {
                    operator, operand, ..
                } if operator == "*" => {
                    let Expression::Identifier(target, _) = operand.as_ref() else {
                        diagnostics.push(unsupported(
                            "Dereferenzierte Zuweisungen benötigen eine lokale Referenz.",
                            *span,
                        ));
                        return None;
                    };
                    (target.clone(), true)
                }
                _ => {
                    diagnostics.push(unsupported(
                        "Zuweisungen sind nur an lokale Namen oder dereferenzierte Referenzen zulässig.",
                        *span,
                    ));
                    return None;
                }
            };
            Some(HirExpression::Assignment {
                target,
                through_reference,
                operator: operator.clone(),
                value: Box::new(lower_expression(value, signatures, diagnostics)?),
                span: *span,
            })
        }
        Expression::Binary {
            left,
            operator,
            right,
            span,
        } => Some(HirExpression::Binary {
            left: Box::new(lower_expression(left, signatures, diagnostics)?),
            operator: operator.clone(),
            right: Box::new(lower_expression(right, signatures, diagnostics)?),
            span: *span,
        }),
        Expression::Call {
            callee,
            generic_arguments,
            arguments,
            span,
        } => {
            let (name, intrinsic) = match callee.as_ref() {
                Expression::Identifier(name, _) if signatures.has_method_in_current_class(name) => {
                    return lower_method_call(
                        None,
                        name,
                        generic_arguments,
                        arguments,
                        *span,
                        signatures,
                        diagnostics,
                    );
                }
                Expression::Identifier(name, _) => (name.clone(), false),
                Expression::Member { object, name, .. }
                    if matches!(object.as_ref(), Expression::Identifier(owner, _) if owner == "Console")
                        && matches!(name.as_str(), "writeLine" | "write") =>
                {
                    (format!("Console.{name}"), true)
                }
                Expression::Member { object, name, .. }
                    if matches!(object.as_ref(), Expression::Identifier(owner, _)
                        if builtins::is_namespace(owner) || (owner == "Console" && name == "readLine")) =>
                {
                    let Expression::Identifier(owner, _) = object.as_ref() else {
                        unreachable!("Guard prüft den Identifier");
                    };
                    let full_name = format!("{owner}.{name}");
                    let Some(builtin) = builtins::find(&full_name) else {
                        diagnostics.push(unsupported(
                            format!("'{full_name}' ist keine bekannte eingebaute Funktion."),
                            *span,
                        ));
                        return None;
                    };
                    if arguments.len() != builtin.params.len() {
                        diagnostics.push(unsupported(
                            format!(
                                "'{full_name}' erwartet {} Argumente, übergeben wurden {}.",
                                builtin.params.len(),
                                arguments.len()
                            ),
                            *span,
                        ));
                        return None;
                    }
                    return Some(HirExpression::Call {
                        callee: full_name,
                        arguments: arguments
                            .iter()
                            .map(|argument| lower_expression(argument, signatures, diagnostics))
                            .collect::<Option<Vec<_>>>()?,
                        return_type: builtin.returns.to_owned(),
                        span: *span,
                    });
                }
                Expression::Member { object, name, .. } => {
                    return lower_method_call(
                        Some(object),
                        name,
                        generic_arguments,
                        arguments,
                        *span,
                        signatures,
                        diagnostics,
                    );
                }
                _ => {
                    diagnostics.push(unsupported(
                        "Das MIR-Subset unterstützt derzeit nur direkte freie Funktionen und Console-Ausgabe.",
                        *span,
                    ));
                    return None;
                }
            };
            if !generic_arguments.is_empty() {
                diagnostics.push(unsupported(
                    "Generische Funktionsaufrufe benötigen noch Monomorphisierung.",
                    *span,
                ));
                return None;
            }
            if intrinsic {
                if arguments.len() != 1 {
                    diagnostics.push(unsupported(
                        "Console.write und Console.writeLine benötigen genau einen Stringwert.",
                        *span,
                    ));
                    return None;
                }
                return Some(HirExpression::Call {
                    callee: name,
                    arguments: arguments
                        .iter()
                        .map(|argument| lower_expression(argument, signatures, diagnostics))
                        .collect::<Option<Vec<_>>>()?,
                    return_type: "void".to_owned(),
                    span: *span,
                });
            }

            let Some(candidates) = signatures.get(&name) else {
                diagnostics.push(unsupported(
                    format!("Funktion '{name}' ist im MIR-Subset nicht auflösbar."),
                    *span,
                ));
                return None;
            };
            let mut matching = candidates
                .iter()
                .filter(|candidate| candidate.parameter_count == arguments.len());
            let Some(signature) = matching.next() else {
                diagnostics.push(unsupported(
                    format!(
                        "Aufruf von '{name}' hat {} Argumente, aber keine unterstützte Signatur.",
                        arguments.len()
                    ),
                    *span,
                ));
                return None;
            };
            if matching.next().is_some() {
                diagnostics.push(unsupported(
                    format!(
                        "Überladener Aufruf von '{name}' ist im MIR-Subset nicht eindeutig auflösbar."
                    ),
                    *span,
                ));
                return None;
            }
            if signature.is_generic {
                diagnostics.push(unsupported(
                    format!("Generische Funktion '{name}' benötigt noch Monomorphisierung."),
                    *span,
                ));
                return None;
            }

            Some(HirExpression::Call {
                callee: name.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| lower_expression(argument, signatures, diagnostics))
                    .collect::<Option<Vec<_>>>()?,
                return_type: signature.return_type.clone(),
                span: *span,
            })
        }
        _ => {
            diagnostics.push(unsupported(
                "Dieser Ausdruck wird in der aktuellen Lowering-Stufe nicht unterstützt.",
                expression.span(),
            ));
            None
        }
    }
}

fn resolve_field(
    object: &HirExpression,
    name: &str,
    span: SourceSpan,
    env: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, String)> {
    let class = env.object_class(object, span, diagnostics)?;
    let field = env.classes[&class]
        .fields
        .iter()
        .find(|(field, _)| field == name);
    let Some((_, field_type)) = field else {
        diagnostics.push(unsupported(
            format!("Klasse '{class}' besitzt kein Feld '{name}'."),
            span,
        ));
        return None;
    };
    Some((class, field_type.clone()))
}

#[allow(clippy::too_many_arguments)]
fn lower_field_assignment(
    object: HirExpression,
    class: String,
    field: &str,
    type_name: String,
    operator: &str,
    value: &Expression,
    span: SourceSpan,
    env: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HirExpression> {
    Some(HirExpression::FieldAssignment {
        object: Box::new(object),
        class,
        field: field.to_owned(),
        type_name,
        operator: operator.to_owned(),
        value: Box::new(lower_expression(value, env, diagnostics)?),
        span,
    })
}

/// Lowers `name(args)` (implicit receiver) or `object.name(args)` / `Class.name(args)`
/// on a native class into a call of `Class.name` with `this` as first argument.
fn lower_method_call(
    receiver: Option<&Expression>,
    name: &str,
    generic_arguments: &[TypeReference],
    arguments: &[Expression],
    span: SourceSpan,
    env: &LowerEnv,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HirExpression> {
    let (class, receiver) = match receiver {
        None => {
            let context = env.current_context()?;
            let this = context
                .instance
                .then(|| HirExpression::Identifier("this".to_owned(), span));
            (context.class, this)
        }
        Some(Expression::Identifier(owner, _))
            if env.lookup(owner).is_none()
                && (env.classes.contains_key(owner)
                    || env.unsupported_classes.contains_key(owner)) =>
        {
            if env.reject_unsupported_class(owner, span, diagnostics) {
                return None;
            }
            (owner.clone(), None)
        }
        Some(object) => {
            let object = lower_expression(object, env, diagnostics)?;
            (env.object_class(&object, span, diagnostics)?, Some(object))
        }
    };
    let resolved_name = if generic_arguments.is_empty() {
        name.to_owned()
    } else {
        specialize_method_name(name, generic_arguments)
    };
    let Some(method) = env
        .classes
        .get(&class)
        .and_then(|info| info.methods.get(&resolved_name))
    else {
        diagnostics.push(unsupported(
            format!("Klasse '{class}' besitzt keine passende Methode '{name}'."),
            span,
        ));
        return None;
    };
    if method.parameter_count != arguments.len() {
        diagnostics.push(unsupported(
            format!(
                "Aufruf von '{class}.{name}' hat {} Argumente, erwartet werden {}.",
                arguments.len(),
                method.parameter_count
            ),
            span,
        ));
        return None;
    }
    let mut lowered_arguments = Vec::with_capacity(arguments.len() + 1);
    if !method.is_static {
        let Some(receiver) = receiver else {
            diagnostics.push(unsupported(
                format!("Instanzmethode '{class}.{name}' benötigt ein Objekt."),
                span,
            ));
            return None;
        };
        lowered_arguments.push(receiver);
    }
    for argument in arguments {
        lowered_arguments.push(lower_expression(argument, env, diagnostics)?);
    }
    let callee = if method.is_virtual && !method.is_static {
        let mut targets = env
            .classes
            .iter()
            .filter(|(candidate, _)| is_class_subtype(&env.classes, candidate, &class))
            .filter_map(|(_, candidate_info)| {
                let implementation = candidate_info.methods.get(&resolved_name)?;
                Some(format!(
                    "{}={}",
                    candidate_info.class_id, implementation.function_name
                ))
            })
            .collect::<Vec<_>>();
        targets.sort();
        format!(
            "__nortex_virtual|{}|{}",
            method.function_name,
            targets.join("|")
        )
    } else {
        method.function_name.clone()
    };
    Some(HirExpression::Call {
        callee,
        arguments: lowered_arguments,
        return_type: method.return_type.clone(),
        span,
    })
}

fn is_class_subtype(classes: &HashMap<String, ClassInfo>, actual: &str, expected: &str) -> bool {
    let mut current = Some(actual);
    while let Some(name) = current {
        if name == expected {
            return true;
        }
        current = classes
            .get(name)
            .and_then(|info| info.base_class.as_deref());
    }
    false
}

fn type_name(type_ref: &TypeReference) -> String {
    let mut name = type_ref.name.clone();
    if !type_ref.generic_arguments.is_empty() {
        name.push('<');
        name.push_str(
            &type_ref
                .generic_arguments
                .iter()
                .map(type_name)
                .collect::<Vec<_>>()
                .join(", "),
        );
        name.push('>');
    }
    name.push_str(&"[]".repeat(type_ref.array_depth));
    if let Some(length) = type_ref.fixed_array_length {
        name.push_str(&format!("[{length}]"));
    }
    match type_ref.reference_kind {
        ReferenceKind::Value => name,
        ReferenceKind::SharedBorrow => format!("&{name}"),
        ReferenceKind::MutableBorrow => format!("&mut {name}"),
        ReferenceKind::RawConstPointer => format!("*const {name}"),
        ReferenceKind::RawMutablePointer => format!("*mut {name}"),
    }
}

fn specialize_generic_classes(program: &Program) -> Vec<ClassDeclaration> {
    let templates: HashMap<_, _> = program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Class(class)
                if class.kind == TypeDeclarationKind::Class
                    && !class.generic_parameters.is_empty() =>
            {
                Some((class.name.as_str(), class))
            }
            _ => None,
        })
        .collect();
    if templates.is_empty() {
        return Vec::new();
    }

    let mut instances = HashMap::<String, HashMap<String, Vec<TypeReference>>>::new();
    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                if let Some(body) = &function.body {
                    collect_block_instantiations(body, &mut instances);
                }
            }
            Declaration::Class(class) => {
                for member in &class.members {
                    match member {
                        MemberDeclaration::Field(field) => {
                            if let Some(initializer) = &field.initializer {
                                collect_expression_instantiations(initializer, &mut instances);
                            }
                        }
                        MemberDeclaration::Method(method) => {
                            if let Some(body) = &method.body {
                                collect_block_instantiations(body, &mut instances);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Declaration::GlobalVariable(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expression_instantiations(initializer, &mut instances);
                }
            }
        }
    }

    let mut specialized = Vec::new();
    for (name, template) in templates {
        let Some(class_instances) = instances.get(name) else {
            continue;
        };
        for arguments in class_instances.values() {
            if arguments.len() != template.generic_parameters.len()
                || arguments.iter().any(|argument| {
                    contains_generic_parameter(argument, &template.generic_parameters)
                })
            {
                continue;
            }
            let substitutions: HashMap<_, _> = template
                .generic_parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            let specialized_name = format!(
                "{}<{}>",
                name,
                arguments
                    .iter()
                    .map(type_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let mut class = template.clone();
            class.name = specialized_name.clone();
            class.generic_parameters.clear();
            for base in &mut class.base_types {
                *base = substitute_type(base, &substitutions);
            }
            for member in &mut class.members {
                match member {
                    MemberDeclaration::Field(field) => {
                        field.type_ref = substitute_type(&field.type_ref, &substitutions);
                        if let Some(initializer) = &mut field.initializer {
                            substitute_expression(initializer, &substitutions);
                        }
                    }
                    MemberDeclaration::Method(method) => {
                        if method.name == name {
                            method.name = specialized_name.clone();
                        }
                        substitute_function(method, &substitutions);
                    }
                    _ => {}
                }
            }
            specialized.push(class);
        }
    }
    specialized
}

fn specialize_generic_methods(program: &Program, classes: &mut [ClassDeclaration]) {
    let mut instances = HashMap::<String, HashMap<String, Vec<TypeReference>>>::new();
    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                if let Some(body) = &function.body {
                    collect_generic_method_calls_block(body, &mut instances);
                }
            }
            Declaration::Class(class) => {
                for member in &class.members {
                    match member {
                        MemberDeclaration::Field(field) => {
                            if let Some(value) = &field.initializer {
                                collect_generic_method_calls_expression(value, &mut instances);
                            }
                        }
                        MemberDeclaration::Method(method) => {
                            if let Some(body) = &method.body {
                                collect_generic_method_calls_block(body, &mut instances);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Declaration::GlobalVariable(field) => {
                if let Some(value) = &field.initializer {
                    collect_generic_method_calls_expression(value, &mut instances);
                }
            }
        }
    }

    for class in classes {
        let mut members = Vec::new();
        for member in std::mem::take(&mut class.members) {
            let MemberDeclaration::Method(method) = member else {
                members.push(member);
                continue;
            };
            if method.generic_parameters.is_empty() {
                members.push(MemberDeclaration::Method(method));
                continue;
            }
            let Some(specializations) = instances.get(&method.name) else {
                continue;
            };
            for arguments in specializations.values() {
                if arguments.len() != method.generic_parameters.len()
                    || arguments.iter().any(|argument| {
                        contains_generic_parameter(argument, &method.generic_parameters)
                    })
                {
                    continue;
                }
                let substitutions: HashMap<_, _> = method
                    .generic_parameters
                    .iter()
                    .cloned()
                    .zip(arguments.iter().cloned())
                    .collect();
                let mut specialized = method.clone();
                specialized.name = specialize_method_name(&method.name, arguments);
                specialized.generic_parameters.clear();
                substitute_function(&mut specialized, &substitutions);
                members.push(MemberDeclaration::Method(specialized));
            }
        }
        class.members = members;
    }
}

fn specialize_method_name(name: &str, arguments: &[TypeReference]) -> String {
    let suffix = arguments
        .iter()
        .map(type_name)
        .map(|name| {
            name.chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || character == '_' {
                        character
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("$");
    format!("{name}${suffix}")
}

fn collect_generic_method_calls_block(
    block: &BlockStatement,
    instances: &mut HashMap<String, HashMap<String, Vec<TypeReference>>>,
) {
    for statement in &block.statements {
        collect_generic_method_calls_statement(statement, instances);
    }
}

fn collect_generic_method_calls_statement(
    statement: &Statement,
    instances: &mut HashMap<String, HashMap<String, Vec<TypeReference>>>,
) {
    match statement {
        Statement::Block(block) => collect_generic_method_calls_block(block, instances),
        Statement::Variable(variable) => {
            if let Some(value) = &variable.initializer {
                collect_generic_method_calls_expression(value, instances);
            }
        }
        Statement::Return(value, _) | Statement::Throw(value, _) => {
            if let Some(value) = value {
                collect_generic_method_calls_expression(value, instances);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_generic_method_calls_expression(condition, instances);
            collect_generic_method_calls_statement(then_branch, instances);
            if let Some(branch) = else_branch {
                collect_generic_method_calls_statement(branch, instances);
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            collect_generic_method_calls_expression(condition, instances);
            collect_generic_method_calls_statement(body, instances);
        }
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                collect_generic_method_calls_statement(initializer, instances);
            }
            if let Some(condition) = condition {
                collect_generic_method_calls_expression(condition, instances);
            }
            if let Some(increment) = increment {
                collect_generic_method_calls_expression(increment, instances);
            }
            collect_generic_method_calls_statement(body, instances);
        }
        Statement::ForEach { iterable, body, .. } => {
            collect_generic_method_calls_expression(iterable, instances);
            collect_generic_method_calls_statement(body, instances);
        }
        Statement::Switch { value, cases, .. } => {
            collect_generic_method_calls_expression(value, instances);
            for case in cases {
                if let Some(value) = &case.value {
                    collect_generic_method_calls_expression(value, instances);
                }
                for statement in &case.statements {
                    collect_generic_method_calls_statement(statement, instances);
                }
            }
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            collect_generic_method_calls_block(body, instances);
            for catch in catches {
                collect_generic_method_calls_block(&catch.body, instances);
            }
            if let Some(block) = finally_block {
                collect_generic_method_calls_block(block, instances);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_generic_method_calls_expression(value, instances);
            for arm in arms {
                collect_generic_method_calls_block(&arm.body, instances);
            }
        }
        Statement::Expression(expression, _) => {
            collect_generic_method_calls_expression(expression, instances);
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
    }
}

fn collect_generic_method_calls_expression(
    expression: &Expression,
    instances: &mut HashMap<String, HashMap<String, Vec<TypeReference>>>,
) {
    match expression {
        Expression::Unary { operand, .. } | Expression::Group(operand, _) => {
            collect_generic_method_calls_expression(operand, instances);
        }
        Expression::Binary { left, right, .. }
        | Expression::Assignment {
            target: left,
            value: right,
            ..
        } => {
            collect_generic_method_calls_expression(left, instances);
            collect_generic_method_calls_expression(right, instances);
        }
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            collect_generic_method_calls_expression(condition, instances);
            collect_generic_method_calls_expression(when_true, instances);
            collect_generic_method_calls_expression(when_false, instances);
        }
        Expression::Call {
            callee,
            generic_arguments,
            arguments,
            ..
        } => {
            if !generic_arguments.is_empty() {
                let name = match callee.as_ref() {
                    Expression::Member { name, .. } | Expression::Identifier(name, _) => name,
                    _ => "",
                };
                if !name.is_empty() {
                    let key = generic_arguments
                        .iter()
                        .map(type_name)
                        .collect::<Vec<_>>()
                        .join(", ");
                    instances
                        .entry(name.to_owned())
                        .or_default()
                        .entry(key)
                        .or_insert_with(|| generic_arguments.clone());
                }
            }
            collect_generic_method_calls_expression(callee, instances);
            for argument in arguments {
                collect_generic_method_calls_expression(argument, instances);
            }
        }
        Expression::ObjectCreation { arguments, .. } => {
            for argument in arguments {
                collect_generic_method_calls_expression(argument, instances);
            }
        }
        Expression::Member { object, .. } => {
            collect_generic_method_calls_expression(object, instances);
        }
        Expression::Index { object, index, .. } => {
            collect_generic_method_calls_expression(object, instances);
            collect_generic_method_calls_expression(index, instances);
        }
        Expression::Identifier(_, _)
        | Expression::Integer(_, _)
        | Expression::Float(_, _)
        | Expression::String(_, _)
        | Expression::Character(_, _)
        | Expression::Boolean(_, _)
        | Expression::Null(_) => {}
    }
}

fn contains_generic_parameter(ty: &TypeReference, parameters: &[String]) -> bool {
    parameters.iter().any(|parameter| parameter == &ty.name)
        || ty
            .generic_arguments
            .iter()
            .any(|argument| contains_generic_parameter(argument, parameters))
}

fn substitute_type(
    ty: &TypeReference,
    substitutions: &HashMap<String, TypeReference>,
) -> TypeReference {
    if let Some(replacement) = substitutions.get(&ty.name)
        && ty.generic_arguments.is_empty()
        && ty.array_depth == 0
        && ty.fixed_array_length.is_none()
        && ty.reference_kind == ReferenceKind::Value
    {
        return replacement.clone();
    }
    let mut substituted = ty.clone();
    substituted.generic_arguments = ty
        .generic_arguments
        .iter()
        .map(|argument| substitute_type(argument, substitutions))
        .collect();
    substituted
}

fn substitute_function(
    function: &mut FunctionDeclaration,
    substitutions: &HashMap<String, TypeReference>,
) {
    function.return_type = substitute_type(&function.return_type, substitutions);
    for parameter in &mut function.parameters {
        parameter.type_ref = substitute_type(&parameter.type_ref, substitutions);
    }
    if let Some(body) = &mut function.body {
        substitute_block(body, substitutions);
    }
}

fn substitute_block(block: &mut BlockStatement, substitutions: &HashMap<String, TypeReference>) {
    for statement in &mut block.statements {
        substitute_statement(statement, substitutions);
    }
}

fn substitute_statement(statement: &mut Statement, substitutions: &HashMap<String, TypeReference>) {
    match statement {
        Statement::Block(block) => substitute_block(block, substitutions),
        Statement::Variable(variable) => {
            if let Some(ty) = &mut variable.type_ref {
                *ty = substitute_type(ty, substitutions);
            }
            if let Some(value) = &mut variable.initializer {
                substitute_expression(value, substitutions);
            }
        }
        Statement::Return(value, _) | Statement::Throw(value, _) => {
            if let Some(value) = value {
                substitute_expression(value, substitutions);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            substitute_expression(condition, substitutions);
            substitute_statement(then_branch, substitutions);
            if let Some(branch) = else_branch {
                substitute_statement(branch, substitutions);
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            substitute_expression(condition, substitutions);
            substitute_statement(body, substitutions);
        }
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                substitute_statement(initializer, substitutions);
            }
            if let Some(condition) = condition {
                substitute_expression(condition, substitutions);
            }
            if let Some(increment) = increment {
                substitute_expression(increment, substitutions);
            }
            substitute_statement(body, substitutions);
        }
        Statement::ForEach {
            variable_type,
            iterable,
            body,
            ..
        } => {
            *variable_type = substitute_type(variable_type, substitutions);
            substitute_expression(iterable, substitutions);
            substitute_statement(body, substitutions);
        }
        Statement::Switch { value, cases, .. } => {
            substitute_expression(value, substitutions);
            for case in cases {
                if let Some(value) = &mut case.value {
                    substitute_expression(value, substitutions);
                }
                for statement in &mut case.statements {
                    substitute_statement(statement, substitutions);
                }
            }
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            substitute_block(body, substitutions);
            for catch in catches {
                if let Some(ty) = &mut catch.exception_type {
                    *ty = substitute_type(ty, substitutions);
                }
                substitute_block(&mut catch.body, substitutions);
            }
            if let Some(finally_block) = finally_block {
                substitute_block(finally_block, substitutions);
            }
        }
        Statement::Match { value, arms, .. } => {
            substitute_expression(value, substitutions);
            for arm in arms {
                substitute_block(&mut arm.body, substitutions);
            }
        }
        Statement::Expression(expression, _) => {
            substitute_expression(expression, substitutions);
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
    }
}

fn substitute_expression(
    expression: &mut Expression,
    substitutions: &HashMap<String, TypeReference>,
) {
    match expression {
        Expression::Unary { operand, .. } | Expression::Group(operand, _) => {
            substitute_expression(operand, substitutions);
        }
        Expression::Binary { left, right, .. }
        | Expression::Assignment {
            target: left,
            value: right,
            ..
        } => {
            substitute_expression(left, substitutions);
            substitute_expression(right, substitutions);
        }
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            substitute_expression(condition, substitutions);
            substitute_expression(when_true, substitutions);
            substitute_expression(when_false, substitutions);
        }
        Expression::Call {
            callee,
            generic_arguments,
            arguments,
            ..
        } => {
            substitute_expression(callee, substitutions);
            for argument in generic_arguments {
                *argument = substitute_type(argument, substitutions);
            }
            for argument in arguments {
                substitute_expression(argument, substitutions);
            }
        }
        Expression::ObjectCreation {
            type_ref,
            arguments,
            ..
        } => {
            *type_ref = substitute_type(type_ref, substitutions);
            for argument in arguments {
                substitute_expression(argument, substitutions);
            }
        }
        Expression::Member { object, .. } => substitute_expression(object, substitutions),
        Expression::Index { object, index, .. } => {
            substitute_expression(object, substitutions);
            substitute_expression(index, substitutions);
        }
        Expression::Identifier(_, _)
        | Expression::Integer(_, _)
        | Expression::Float(_, _)
        | Expression::String(_, _)
        | Expression::Character(_, _)
        | Expression::Boolean(_, _)
        | Expression::Null(_) => {}
    }
}

fn collect_block_instantiations(
    block: &BlockStatement,
    instances: &mut HashMap<String, HashMap<String, Vec<TypeReference>>>,
) {
    for statement in &block.statements {
        collect_statement_instantiations(statement, instances);
    }
}

fn collect_statement_instantiations(
    statement: &Statement,
    instances: &mut HashMap<String, HashMap<String, Vec<TypeReference>>>,
) {
    match statement {
        Statement::Block(block) => collect_block_instantiations(block, instances),
        Statement::Variable(variable) => {
            if let Some(value) = &variable.initializer {
                collect_expression_instantiations(value, instances);
            }
        }
        Statement::Return(value, _) | Statement::Throw(value, _) => {
            if let Some(value) = value {
                collect_expression_instantiations(value, instances);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expression_instantiations(condition, instances);
            collect_statement_instantiations(then_branch, instances);
            if let Some(branch) = else_branch {
                collect_statement_instantiations(branch, instances);
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            collect_expression_instantiations(condition, instances);
            collect_statement_instantiations(body, instances);
        }
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                collect_statement_instantiations(initializer, instances);
            }
            if let Some(condition) = condition {
                collect_expression_instantiations(condition, instances);
            }
            if let Some(increment) = increment {
                collect_expression_instantiations(increment, instances);
            }
            collect_statement_instantiations(body, instances);
        }
        Statement::ForEach { iterable, body, .. } => {
            collect_expression_instantiations(iterable, instances);
            collect_statement_instantiations(body, instances);
        }
        Statement::Switch { value, cases, .. } => {
            collect_expression_instantiations(value, instances);
            for case in cases {
                if let Some(value) = &case.value {
                    collect_expression_instantiations(value, instances);
                }
                for statement in &case.statements {
                    collect_statement_instantiations(statement, instances);
                }
            }
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            collect_block_instantiations(body, instances);
            for catch in catches {
                collect_block_instantiations(&catch.body, instances);
            }
            if let Some(finally_block) = finally_block {
                collect_block_instantiations(finally_block, instances);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expression_instantiations(value, instances);
            for arm in arms {
                collect_block_instantiations(&arm.body, instances);
            }
        }
        Statement::Expression(expression, _) => {
            collect_expression_instantiations(expression, instances);
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
    }
}

fn collect_expression_instantiations(
    expression: &Expression,
    instances: &mut HashMap<String, HashMap<String, Vec<TypeReference>>>,
) {
    match expression {
        Expression::Unary { operand, .. } | Expression::Group(operand, _) => {
            collect_expression_instantiations(operand, instances);
        }
        Expression::Binary { left, right, .. }
        | Expression::Assignment {
            target: left,
            value: right,
            ..
        } => {
            collect_expression_instantiations(left, instances);
            collect_expression_instantiations(right, instances);
        }
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            collect_expression_instantiations(condition, instances);
            collect_expression_instantiations(when_true, instances);
            collect_expression_instantiations(when_false, instances);
        }
        Expression::Call {
            callee, arguments, ..
        } => {
            collect_expression_instantiations(callee, instances);
            for argument in arguments {
                collect_expression_instantiations(argument, instances);
            }
        }
        Expression::ObjectCreation {
            type_ref,
            arguments,
            ..
        } => {
            if !type_ref.generic_arguments.is_empty() {
                let specialization = type_name(type_ref);
                instances
                    .entry(type_ref.name.clone())
                    .or_default()
                    .entry(specialization)
                    .or_insert_with(|| type_ref.generic_arguments.clone());
            }
            for argument in arguments {
                collect_expression_instantiations(argument, instances);
            }
        }
        Expression::Member { object, .. } => {
            collect_expression_instantiations(object, instances);
        }
        Expression::Index { object, index, .. } => {
            collect_expression_instantiations(object, instances);
            collect_expression_instantiations(index, instances);
        }
        Expression::Identifier(_, _)
        | Expression::Integer(_, _)
        | Expression::Float(_, _)
        | Expression::String(_, _)
        | Expression::Character(_, _)
        | Expression::Boolean(_, _)
        | Expression::Null(_) => {}
    }
}

fn statement_span(statement: &Statement) -> SourceSpan {
    match statement {
        Statement::Block(block) => block.span,
        Statement::Variable(variable) => variable.span,
        Statement::Return(_, span)
        | Statement::Throw(_, span)
        | Statement::Break(span)
        | Statement::Continue(span)
        | Statement::Expression(_, span)
        | Statement::Empty(span) => *span,
        Statement::If { span, .. }
        | Statement::While { span, .. }
        | Statement::For { span, .. }
        | Statement::ForEach { span, .. }
        | Statement::Switch { span, .. }
        | Statement::Try { span, .. }
        | Statement::Match { span, .. } => *span,
    }
}

fn unsupported(message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    Diagnostic::error_with_code("NX2001", message, span)
}

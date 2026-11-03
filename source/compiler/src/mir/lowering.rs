//! Lowering from the supported scalar/string/class HIR subset to executable MIR.

use std::collections::{HashMap, HashSet};

use crate::builtins;
use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::hir::{HirBlock, HirClass, HirExpression, HirFunction, HirGlobal, HirStatement};

use super::{
    BasicBlockData, BasicBlockId, BinaryOp, LocalDecl, LocalId, MirBody, MirGlobal, MirGlobalValue,
    Operand, Place, Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
};

pub fn lower_globals(globals: &[HirGlobal]) -> Result<Vec<MirGlobal>, Vec<Diagnostic>> {
    let mut lowered = Vec::with_capacity(globals.len());
    let mut diagnostics = Vec::new();
    for global in globals {
        if !is_mir_scalar(&global.type_name) && global.type_name != "string" {
            diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                format!(
                    "Globaler Typ '{}' wird in der aktuellen MIR-Stufe nicht unterstützt.",
                    global.type_name
                ),
                global.span,
            ));
            continue;
        }
        let Some(value) = evaluate_global_initializer(&global.initializer) else {
            diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                "Der globale Initialisierungswert muss ein unterstützter skalarer Konstantenausdruck sein.",
                global.initializer.span(),
            ));
            continue;
        };
        let value = match (global.type_name.as_str(), value) {
            ("string", ConstantValue::String(value)) => MirGlobalValue::String(value),
            (_, ConstantValue::String(_)) | ("string", _) => {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1102",
                    "Ein string-Global benötigt ein Stringliteral und umgekehrt.",
                    global.initializer.span(),
                ));
                continue;
            }
            ("bool", ConstantValue::Boolean(value)) => MirGlobalValue::Boolean(value),
            ("bool", ConstantValue::Integer(_)) => {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1102",
                    "Ein bool-Global benötigt einen bool-Ausdruck.",
                    global.initializer.span(),
                ));
                continue;
            }
            (_, ConstantValue::Integer(value)) => MirGlobalValue::Integer(value),
            (_, ConstantValue::Boolean(_)) => {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1102",
                    "Ein numerisches Global benötigt einen Zahlen-Ausdruck.",
                    global.initializer.span(),
                ));
                continue;
            }
        };
        if let MirGlobalValue::Integer(value) = value
            && !integer_fits_type(value, &global.type_name)
        {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1102",
                format!(
                    "Der Initialisierungswert liegt außerhalb des Wertebereichs von '{}'.",
                    global.type_name
                ),
                global.initializer.span(),
            ));
            continue;
        }
        lowered.push(MirGlobal {
            name: global.name.clone(),
            type_name: global.type_name.clone(),
            mutable: global.mutable,
            public: global.public,
            value,
        });
    }
    if diagnostics.is_empty() {
        Ok(lowered)
    } else {
        Err(diagnostics)
    }
}

fn integer_fits_type(value: i64, type_name: &str) -> bool {
    let bits = match type_name {
        "byte" | "sbyte" | "i8" | "u8" => 8,
        "short" | "ushort" | "i16" | "u16" => 16,
        "int" | "uint" | "i32" | "u32" => 32,
        "long" | "ulong" | "nint" | "nuint" | "isize" | "usize" | "i64" | "u64" => 64,
        _ => return false,
    };
    let unsigned = matches!(
        type_name,
        "byte" | "ushort" | "uint" | "ulong" | "nuint" | "usize" | "u8" | "u16" | "u32" | "u64"
    );
    if unsigned {
        if value < 0 {
            return false;
        }
        bits == 64 || value < (1_i64 << bits)
    } else if bits == 64 {
        true
    } else {
        let bound = 1_i64 << (bits - 1);
        value >= -bound && value < bound
    }
}

enum ConstantValue {
    Integer(i64),
    Boolean(bool),
    String(String),
}

fn evaluate_global_initializer(expression: &HirExpression) -> Option<ConstantValue> {
    match expression {
        HirExpression::Integer(value, _) => parse_integer(value).map(ConstantValue::Integer),
        HirExpression::Boolean(value, _) => Some(ConstantValue::Boolean(*value)),
        HirExpression::String(value, _) => Some(ConstantValue::String(value.clone())),
        HirExpression::Unary {
            operator, operand, ..
        } => match (operator.as_str(), evaluate_global_initializer(operand)?) {
            ("+", ConstantValue::Integer(value)) => Some(ConstantValue::Integer(value)),
            ("-", ConstantValue::Integer(value)) => value.checked_neg().map(ConstantValue::Integer),
            ("!", ConstantValue::Boolean(value)) => Some(ConstantValue::Boolean(!value)),
            _ => None,
        },
        HirExpression::Binary {
            left,
            operator,
            right,
            ..
        } => match (
            evaluate_global_initializer(left)?,
            operator.as_str(),
            evaluate_global_initializer(right)?,
        ) {
            (ConstantValue::String(left), "+", ConstantValue::String(right)) => {
                Some(ConstantValue::String(left + &right))
            }
            (ConstantValue::Integer(left), operator, ConstantValue::Integer(right)) => {
                let result = match operator {
                    "+" => ConstantValue::Integer(left.checked_add(right)?),
                    "-" => ConstantValue::Integer(left.checked_sub(right)?),
                    "*" => ConstantValue::Integer(left.checked_mul(right)?),
                    "/" => ConstantValue::Integer(left.checked_div(right)?),
                    "%" => ConstantValue::Integer(left.checked_rem(right)?),
                    "&" => ConstantValue::Integer(left & right),
                    "|" => ConstantValue::Integer(left | right),
                    "^" => ConstantValue::Integer(left ^ right),
                    "<<" => ConstantValue::Integer(left.checked_shl(u32::try_from(right).ok()?)?),
                    ">>" => ConstantValue::Integer(left.checked_shr(u32::try_from(right).ok()?)?),
                    "==" => ConstantValue::Boolean(left == right),
                    "!=" => ConstantValue::Boolean(left != right),
                    "<" => ConstantValue::Boolean(left < right),
                    "<=" => ConstantValue::Boolean(left <= right),
                    ">" => ConstantValue::Boolean(left > right),
                    ">=" => ConstantValue::Boolean(left >= right),
                    _ => return None,
                };
                Some(result)
            }
            (ConstantValue::Boolean(left), "&&", ConstantValue::Boolean(right)) => {
                Some(ConstantValue::Boolean(left && right))
            }
            (ConstantValue::Boolean(left), "||", ConstantValue::Boolean(right)) => {
                Some(ConstantValue::Boolean(left || right))
            }
            (ConstantValue::Boolean(left), "==", ConstantValue::Boolean(right)) => {
                Some(ConstantValue::Boolean(left == right))
            }
            (ConstantValue::Boolean(left), "!=", ConstantValue::Boolean(right)) => {
                Some(ConstantValue::Boolean(left != right))
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn lower_function(
    function: &HirFunction,
    globals: &[MirGlobal],
    classes: &[HirClass],
) -> Result<MirBody, Vec<Diagnostic>> {
    let mut lowerer = FunctionLowerer::new(function, globals, classes);
    lowerer.lower_block(&function.body);
    lowerer.finish()
}

struct FunctionLowerer<'a> {
    function: &'a HirFunction,
    body: MirBody,
    /// Native class name -> field types in declaration order.
    classes: HashMap<String, Vec<(String, String)>>,
    class_bases: HashMap<String, Option<String>>,
    class_ids: HashMap<String, usize>,
    scopes: Vec<HashMap<String, LocalId>>,
    scope_locals: Vec<Vec<LocalId>>,
    loops: Vec<LoopTargets>,
    current_block: Option<BasicBlockId>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy)]
struct LoopTargets {
    break_target: BasicBlockId,
    continue_target: BasicBlockId,
    scope_depth: usize,
}

impl<'a> FunctionLowerer<'a> {
    fn new(function: &'a HirFunction, globals: &[MirGlobal], classes: &[HirClass]) -> Self {
        let class_bases = classes
            .iter()
            .map(|class| (class.name.clone(), class.base_class.clone()))
            .collect::<HashMap<_, _>>();
        let class_ids = classes
            .iter()
            .map(|class| (class.name.clone(), class.class_id))
            .collect::<HashMap<_, _>>();
        let class_names = classes
            .iter()
            .map(|class| class.name.as_str())
            .collect::<HashSet<_>>();
        let classes = classes
            .iter()
            .map(|class| {
                (
                    class.name.clone(),
                    class
                        .fields
                        .iter()
                        .map(|field| {
                            let field_type = if class_names.contains(field.type_name.as_str()) {
                                object_type_name(&field.type_name)
                            } else {
                                field.type_name.clone()
                            };
                            (field.name.clone(), field_type)
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut body = MirBody::new(function.parameters.len());
        body.globals = globals.to_vec();
        let mut diagnostics = Vec::new();
        let return_type = if classes.contains_key(&function.return_type) {
            object_type_name(&function.return_type)
        } else {
            function.return_type.clone()
        };
        if return_type != "void" && !is_mir_supported_type(&return_type) {
            diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                format!("Rückgabetyp '{return_type}' liegt außerhalb des skalaren MIR-Subsets."),
                function.span,
            ));
        }
        body.local_decls.push(LocalDecl {
            name: Some("_return".to_owned()),
            type_name: return_type,
            field_types: Vec::new(),
            is_mut: true,
            is_user_variable: false,
            global_symbol: None,
        });
        let mut parameters = HashMap::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            let local = LocalId(index + 1);
            if let Some(field_types) = classes.get(&parameter.type_name) {
                body.local_decls.push(LocalDecl {
                    name: Some(parameter.name.clone()),
                    type_name: object_type_name(&parameter.type_name),
                    field_types: field_types.iter().map(|(_, ty)| ty.clone()).collect(),
                    is_mut: false,
                    is_user_variable: true,
                    global_symbol: None,
                });
                parameters.insert(parameter.name.clone(), local);
                continue;
            }
            if !is_mir_supported_type(&parameter.type_name) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX2002",
                    format!(
                        "Parametertyp '{}' liegt außerhalb des skalaren MIR-Subsets.",
                        parameter.type_name
                    ),
                    parameter.span,
                ));
            }
            body.local_decls.push(LocalDecl {
                name: Some(parameter.name.clone()),
                type_name: parameter.type_name.clone(),
                field_types: Vec::new(),
                is_mut: false,
                is_user_variable: true,
                global_symbol: None,
            });
            parameters.insert(parameter.name.clone(), local);
        }
        for global in globals {
            let local = LocalId(body.local_decls.len());
            body.local_decls.push(LocalDecl {
                name: Some(global.name.clone()),
                type_name: global.type_name.clone(),
                field_types: Vec::new(),
                is_mut: global.mutable,
                is_user_variable: true,
                global_symbol: Some(global.name.clone()),
            });
            parameters.insert(global.name.clone(), local);
        }
        body.basic_blocks.push(BasicBlockData::default());
        Self {
            function,
            body,
            classes,
            class_bases,
            class_ids,
            scopes: vec![parameters],
            scope_locals: vec![Vec::new()],
            loops: Vec::new(),
            current_block: Some(BasicBlockId(0)),
            diagnostics,
        }
    }

    fn lower_block(&mut self, block: &HirBlock) {
        self.scopes.push(HashMap::new());
        self.scope_locals.push(Vec::new());
        for statement in &block.statements {
            if self.current_block.is_none() {
                break;
            }
            self.lower_statement(statement);
        }
        self.end_scope();
    }

    fn lower_statement(&mut self, statement: &HirStatement) {
        match statement {
            HirStatement::Let {
                name,
                type_name,
                mutable,
                initializer,
                span,
            } => {
                if self.classes.contains_key(type_name) {
                    self.lower_object_let(name, type_name, *mutable, initializer.as_ref(), *span);
                    return;
                }
                if !is_mir_supported_type(type_name) {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        format!(
                            "Lokaler Typ '{type_name}' liegt außerhalb des skalaren MIR-Subsets."
                        ),
                        *span,
                    ));
                    return;
                }
                let local = self.allocate_local(name, type_name, *mutable, true);
                self.push_statement(StatementKind::StorageLive(local), *span);
                if let Some(initializer) = initializer {
                    if !self.lower_expression_into(local, initializer, *span) {
                        return;
                    }
                }
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name.clone(), local);
                }
            }
            HirStatement::Return(value, span) => {
                if self.function.return_type == "void" && value.is_some() {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        "Ein void-MIR-Rückgabepfad darf keinen Wert liefern.",
                        *span,
                    ));
                    return;
                }
                if self.function.return_type != "void" && value.is_none() {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        "Ein nicht-void-MIR-Rückgabepfad benötigt einen Wert.",
                        *span,
                    ));
                    return;
                }
                if let Some(value) = value
                    && let Some(operand) = self.lower_expression(value)
                {
                    let operand = self.owned_operand(value, operand, *span);
                    self.push_statement(
                        StatementKind::Assign(MirBody::return_place(), Rvalue::Use(operand)),
                        *span,
                    );
                }
                self.terminate(TerminatorKind::Return, *span);
                self.current_block = None;
            }
            HirStatement::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let Some(_) = self.current_block else {
                    return;
                };
                let then_block = self.new_block();
                let else_block = self.new_block();
                let join_block = self.new_block();
                if !self.lower_condition(condition, then_block, else_block, *span) {
                    return;
                }

                self.current_block = Some(then_block);
                self.lower_statement(then_branch);
                let then_falls_through = self.current_block.is_some();
                if then_falls_through {
                    self.terminate(TerminatorKind::Goto { target: join_block }, *span);
                }

                self.current_block = Some(else_block);
                if let Some(else_branch) = else_branch {
                    self.lower_statement(else_branch);
                }
                let else_falls_through = self.current_block.is_some();
                if else_falls_through {
                    self.terminate(TerminatorKind::Goto { target: join_block }, *span);
                }

                self.current_block = Some(join_block);
                if !then_falls_through && !else_falls_through {
                    self.terminate(TerminatorKind::Unreachable, *span);
                    self.current_block = None;
                }
            }
            HirStatement::While {
                condition,
                body,
                span,
            } => {
                if self.current_block.is_none() {
                    return;
                }
                let condition_block = self.new_block();
                let body_block = self.new_block();
                let exit_block = self.new_block();
                self.terminate(
                    TerminatorKind::Goto {
                        target: condition_block,
                    },
                    *span,
                );

                self.current_block = Some(condition_block);
                if !self.lower_condition(condition, body_block, exit_block, *span) {
                    return;
                }

                self.loops.push(LoopTargets {
                    break_target: exit_block,
                    continue_target: condition_block,
                    scope_depth: self.scopes.len(),
                });
                self.current_block = Some(body_block);
                self.lower_statement(body);
                if self.current_block.is_some() {
                    self.terminate(
                        TerminatorKind::Goto {
                            target: condition_block,
                        },
                        *span,
                    );
                }
                self.loops.pop().expect("while loop target was pushed");
                self.current_block = Some(exit_block);
                if !self.is_block_reachable(exit_block) {
                    self.terminate(TerminatorKind::Unreachable, *span);
                    self.current_block = None;
                }
            }
            HirStatement::For {
                initializer,
                condition,
                increment,
                body,
                span,
            } => self.lower_for(
                initializer.as_deref(),
                condition.as_ref(),
                increment.as_ref(),
                body,
                *span,
            ),
            HirStatement::LoopControl { is_continue, span } => {
                let Some(loop_targets) = self.loops.last().copied() else {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        "break/continue besitzt beim MIR-Lowering kein umgebendes Loop-Ziel.",
                        *span,
                    ));
                    return;
                };
                let target = if *is_continue {
                    loop_targets.continue_target
                } else {
                    loop_targets.break_target
                };
                let exited_locals = self
                    .scope_locals
                    .iter()
                    .skip(loop_targets.scope_depth)
                    .rev()
                    .flat_map(|locals| locals.iter().rev().copied())
                    .collect::<Vec<_>>();
                for local in exited_locals {
                    self.push_statement(StatementKind::StorageDead(local), *span);
                }
                self.terminate(TerminatorKind::Goto { target }, *span);
                self.current_block = None;
            }
            HirStatement::Expression(expression, span) => {
                if let HirExpression::Call {
                    callee,
                    arguments,
                    return_type,
                    span: call_span,
                } = expression
                    && return_type == "void"
                {
                    let Some(callee) = console_runtime_symbol(callee) else {
                        if builtins::find(callee).is_some() {
                            self.diagnostics.push(Diagnostic::error_with_code(
                                "NX2002",
                                "Void-Builtins werden im MIR-Subset nicht unterstützt.",
                                *call_span,
                            ));
                            return;
                        }
                        self.lower_void_call(callee, arguments, *call_span);
                        return;
                    };
                    let [argument] = arguments.as_slice() else {
                        self.diagnostics.push(Diagnostic::error_with_code(
                            "NX2002",
                            "Console-Ausgabe benötigt genau einen Stringwert.",
                            *call_span,
                        ));
                        return;
                    };
                    if self.expression_type(argument).as_deref() != Some("string") {
                        self.diagnostics.push(Diagnostic::error_with_code(
                            "NX2002",
                            "Console-Ausgabe benötigt einen Wert vom Typ 'string'.",
                            argument.span(),
                        ));
                        return;
                    }
                    let Some(argument) = self.lower_expression(argument) else {
                        return;
                    };
                    let target = self.new_block();
                    self.terminate(
                        TerminatorKind::Call {
                            callee: callee.to_owned(),
                            args: vec![argument],
                            destination: MirBody::return_place(),
                            target,
                            cleanup: None,
                        },
                        *call_span,
                    );
                    self.current_block = Some(target);
                    return;
                }
                if matches!(self.lower_expression(expression), Some(Operand::Move(_))) {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        "Ein verworfener Ausdruck darf im MIR-Subset keinen Wert verschieben.",
                        *span,
                    ));
                }
            }
            HirStatement::Block(block) => self.lower_block(block),
        }
    }

    fn lower_void_call(&mut self, callee: &str, arguments: &[HirExpression], span: SourceSpan) {
        let mut lowered = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let Some(operand) = self.lower_expression(argument) else {
                return;
            };
            lowered.push(self.owned_operand(argument, operand, span));
        }
        let target = self.new_block();
        self.terminate(
            TerminatorKind::Call {
                callee: callee.to_owned(),
                args: lowered,
                destination: MirBody::return_place(),
                target,
                cleanup: None,
            },
            span,
        );
        self.current_block = Some(target);
    }

    /// Declares a local class-reference binding and stores the constructed/returned pointer.
    fn lower_object_let(
        &mut self,
        name: &str,
        class: &str,
        mutable: bool,
        initializer: Option<&HirExpression>,
        span: SourceSpan,
    ) {
        let Some(initializer) = initializer else {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                format!("Objektvariable '{name}' muss initialisiert werden."),
                span,
            ));
            return;
        };
        let Some(created_type) = self.expression_type(initializer) else {
            return;
        };
        if !self.is_class_subtype(&created_type, class) {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                format!("Objektvariable '{name}' vom Typ '{class}' benötigt einen kompatiblen Objektwert."),
                span,
            ));
            return;
        }
        let local = self.allocate_local(name, &object_type_name(class), mutable, true);
        self.push_statement(StatementKind::StorageLive(local), span);
        let Some(value) = self.lower_expression(initializer) else {
            return;
        };
        self.push_statement(
            StatementKind::Assign(Place::from_local(local), Rvalue::Use(value)),
            span,
        );
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), local);
        }
    }

    /// Resolves `object.field` to a field projection of the object's local.
    fn field_place(
        &mut self,
        object: &HirExpression,
        class: &str,
        field: &str,
        span: SourceSpan,
    ) -> Option<Place> {
        let HirExpression::Identifier(name, _) = object else {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                "Feldzugriffe sind nur auf lokalen Objekten und 'this' möglich.",
                span,
            ));
            return None;
        };
        let local = self.resolve_local(name)?;
        let position = self
            .classes
            .get(class)
            .and_then(|fields| fields.iter().position(|(candidate, _)| candidate == field));
        let Some(position) = position
            .filter(|_| self.body.local_decls[local.0].type_name == object_type_name(class))
        else {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                format!(
                    "Feld '{field}' von '{class}' konnte beim MIR-Lowering nicht aufgelöst werden."
                ),
                span,
            ));
            return None;
        };
        Some(Place::from_local(local).project_field(super::FieldIndex(position)))
    }

    fn lower_new(
        &mut self,
        class: &str,
        arguments: &[HirExpression],
        span: SourceSpan,
    ) -> Option<Operand> {
        let Some(fields) = self.classes.get(class).cloned() else {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                format!("Klasse '{class}' ist im MIR nicht auflösbar."),
                span,
            ));
            return None;
        };
        let object = self.new_temp(&object_type_name(class));
        self.body.local_decls[object.0].field_types =
            fields.iter().map(|(_, ty)| ty.clone()).collect();
        let size = (fields.len().max(1) * 8) as i64;
        let class_id = self.class_ids.get(class).copied().unwrap_or_default() as i64;
        let target = self.new_block();
        self.terminate(
            TerminatorKind::Call {
                callee: "nortex_object_alloc".to_owned(),
                args: vec![Operand::ConstantInt(size), Operand::ConstantInt(class_id)],
                destination: Place::from_local(object),
                target,
                cleanup: None,
            },
            span,
        );
        self.current_block = Some(target);
        self.push_statement(
            StatementKind::Assign(
                Place::from_local(object),
                Rvalue::Aggregate(fields.iter().map(|(_, ty)| zero_operand(ty)).collect()),
            ),
            span,
        );
        let mut constructor_arguments = Vec::with_capacity(arguments.len() + 1);
        constructor_arguments.push(Operand::Copy(Place::from_local(object)));
        for argument in arguments {
            let operand = self.lower_expression(argument)?;
            constructor_arguments.push(self.owned_operand(argument, operand, span));
        }
        let target = self.new_block();
        self.terminate(
            TerminatorKind::Call {
                callee: format!("{class}.new"),
                args: constructor_arguments,
                destination: MirBody::return_place(),
                target,
                cleanup: None,
            },
            span,
        );
        self.current_block = Some(target);
        Some(Operand::Copy(Place::from_local(object)))
    }

    fn mir_type_name(&self, source_type: &str) -> String {
        if self.classes.contains_key(source_type) {
            object_type_name(source_type)
        } else {
            source_type.to_owned()
        }
    }

    fn is_class_subtype(&self, actual: &str, expected: &str) -> bool {
        let mut current = Some(actual);
        while let Some(name) = current {
            if name == expected {
                return true;
            }
            current = self.class_bases.get(name).and_then(Option::as_deref);
        }
        false
    }

    fn lower_for(
        &mut self,
        initializer: Option<&HirStatement>,
        condition: Option<&HirExpression>,
        increment: Option<&HirExpression>,
        body: &HirStatement,
        span: SourceSpan,
    ) {
        if self.current_block.is_none() {
            return;
        }
        self.scopes.push(HashMap::new());
        self.scope_locals.push(Vec::new());
        if let Some(initializer) = initializer {
            self.lower_statement(initializer);
        }
        if self.current_block.is_none() {
            self.end_scope();
            return;
        }

        let condition_block = self.new_block();
        let body_block = self.new_block();
        let increment_block = self.new_block();
        let exit_block = self.new_block();
        self.terminate(
            TerminatorKind::Goto {
                target: condition_block,
            },
            span,
        );

        self.current_block = Some(condition_block);
        let condition_lowered = if let Some(condition) = condition {
            self.lower_condition(condition, body_block, exit_block, span)
        } else {
            self.terminate(TerminatorKind::Goto { target: body_block }, span);
            self.current_block = None;
            true
        };
        if !condition_lowered {
            self.end_scope();
            return;
        }
        self.loops.push(LoopTargets {
            break_target: exit_block,
            continue_target: increment_block,
            scope_depth: self.scopes.len(),
        });
        self.current_block = Some(body_block);
        self.lower_statement(body);
        if self.current_block.is_some() {
            self.terminate(
                TerminatorKind::Goto {
                    target: increment_block,
                },
                span,
            );
        }

        self.current_block = Some(increment_block);
        if let Some(increment) = increment
            && matches!(self.lower_expression(increment), Some(Operand::Move(_)))
        {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                "Ein for-Inkrement darf im MIR-Subset keinen Wert verschieben.",
                increment.span(),
            ));
        }
        if self.current_block.is_some() {
            self.terminate(
                TerminatorKind::Goto {
                    target: condition_block,
                },
                span,
            );
        }
        self.loops.pop().expect("for loop target was pushed");
        self.current_block = Some(exit_block);
        if !self.is_block_reachable(exit_block) {
            self.terminate(TerminatorKind::Unreachable, span);
            self.current_block = None;
        }
        self.end_scope();
    }

    fn lower_expression(&mut self, expression: &HirExpression) -> Option<Operand> {
        match expression {
            HirExpression::Identifier(name, span) => {
                let Some(local) = self.resolve_local(name) else {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        format!("Lokale Bindung '{name}' konnte beim MIR-Lowering nicht aufgelöst werden."),
                        *span,
                    ));
                    return None;
                };
                let place = Place::from_local(local);
                Some(if is_copy_type(&self.body.local_decls[local.0].type_name) {
                    Operand::Copy(place)
                } else {
                    Operand::Move(place)
                })
            }
            HirExpression::Field {
                object,
                class,
                field,
                type_name,
                span,
            } => {
                let place = self.field_place(object, class, field, *span)?;
                let temp = self.new_temp(&self.mir_type_name(type_name));
                self.push_statement(
                    StatementKind::Assign(
                        Place::from_local(temp),
                        Rvalue::Use(Operand::Copy(place)),
                    ),
                    *span,
                );
                Some(Operand::Copy(Place::from_local(temp)))
            }
            HirExpression::FieldAssignment {
                object,
                class,
                field,
                type_name,
                operator,
                value,
                span,
            } => {
                let place = self.field_place(object, class, field, *span)?;
                let value_operand = self.lower_expression(value)?;
                let value_operand = self.owned_operand(value, value_operand, *span);
                let assigned = if operator == "=" {
                    Rvalue::Use(value_operand)
                } else {
                    let Some(op) = compound_operator(operator) else {
                        self.unsupported_expression(
                            "Zuweisungsoperator wird im MIR-Subset nicht unterstützt",
                            *span,
                        );
                        return None;
                    };
                    Rvalue::BinaryOp(op, Operand::Copy(place.clone()), value_operand)
                };
                self.push_statement(StatementKind::Assign(place, assigned), *span);
                let temp = self.new_temp(&self.mir_type_name(type_name));
                let place = self.field_place(object, class, field, *span)?;
                self.push_statement(
                    StatementKind::Assign(
                        Place::from_local(temp),
                        Rvalue::Use(Operand::Copy(place)),
                    ),
                    *span,
                );
                Some(Operand::Copy(Place::from_local(temp)))
            }
            HirExpression::New {
                class,
                arguments,
                span,
            } => self.lower_new(class, arguments, *span),
            HirExpression::Integer(value, span) => match parse_integer(value) {
                Some(value) => Some(Operand::ConstantInt(value)),
                None => {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        format!("Integerliteral '{value}' passt nicht in den MIR-Zieltyp i64."),
                        *span,
                    ));
                    None
                }
            },
            HirExpression::Float(value, span) => {
                match value.replace('_', "").trim_end_matches(['d', 'D']).parse() {
                    Ok(value) => Some(Operand::ConstantFloat(value)),
                    Err(_) => {
                        self.diagnostics.push(Diagnostic::error_with_code(
                            "NX2002",
                            format!("Gleitkommaliteral '{value}' ist ungültig oder kein double."),
                            *span,
                        ));
                        None
                    }
                }
            }
            HirExpression::String(value, _) => Some(Operand::ConstantString(value.clone())),
            HirExpression::Boolean(value, _) => Some(Operand::ConstantBool(*value)),
            HirExpression::Call {
                callee,
                arguments,
                return_type,
                span,
            } => {
                if return_type == "void" {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        "Ein void-Funktionsaufruf kann nur als eigenständige Anweisung verwendet werden.",
                        *span,
                    ));
                    return None;
                }
                let builtin = builtins::find(callee);
                if let Some(builtin) = builtin {
                    for (argument, expected) in arguments.iter().zip(builtin.params) {
                        let actual = self.expression_type(argument);
                        if actual.as_deref() != Some(*expected) {
                            self.diagnostics.push(Diagnostic::error_with_code(
                                "NX2002",
                                format!(
                                    "'{callee}' erwartet ein Argument vom Typ '{expected}', gefunden wurde '{}'.",
                                    actual.as_deref().unwrap_or("unbekannt")
                                ),
                                *span,
                            ));
                            return None;
                        }
                    }
                }
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        let operand = self.lower_expression(argument)?;
                        // Builtins leihen ihre Strings nur aus; der Besitz bleibt beim Aufrufer.
                        Some(if builtin.is_some() {
                            operand
                        } else {
                            self.owned_operand(argument, operand, *span)
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let destination = self.new_temp(&self.mir_type_name(return_type));
                let target = self.new_block();
                self.terminate(
                    TerminatorKind::Call {
                        callee: callee.clone(),
                        args: arguments,
                        destination: Place::from_local(destination),
                        target,
                        cleanup: None,
                    },
                    *span,
                );
                self.current_block = Some(target);
                Some(Operand::Copy(Place::from_local(destination)))
            }
            HirExpression::Conditional {
                condition,
                when_true,
                when_false,
                span,
            } => self.lower_conditional_expression(condition, when_true, when_false, *span),
            HirExpression::Assignment {
                target,
                through_reference,
                operator,
                value,
                span,
            } => {
                let Some(local) = self.resolve_local(target) else {
                    self.diagnostics.push(Diagnostic::error_with_code(
                        "NX2002",
                        format!("Zuweisungsziel '{target}' konnte beim MIR-Lowering nicht aufgelöst werden."),
                        *span,
                    ));
                    return None;
                };
                let local_type = &self.body.local_decls[local.0].type_name;
                let target_place = if *through_reference {
                    if !local_type.starts_with("&mut ") {
                        self.diagnostics.push(Diagnostic::error_with_code(
                            "NX2002",
                            format!(
                                "Zuweisung durch '{target}' erfordert eine veränderliche Referenz."
                            ),
                            *span,
                        ));
                        return None;
                    }
                    Place::from_local(local).project_deref()
                } else {
                    if !self.body.local_decls[local.0].is_mut {
                        self.diagnostics.push(Diagnostic::error_with_code(
                            "NX2002",
                            format!("Zuweisungsziel '{target}' ist im MIR nicht veränderlich."),
                            *span,
                        ));
                        return None;
                    }
                    Place::from_local(local)
                };
                if !*through_reference && is_reference_type(local_type) {
                    if !self.lower_expression_into(local, value, *span) {
                        return None;
                    }
                    return Some(Operand::Copy(Place::from_local(local)));
                }
                let value_operand = self.lower_expression(value)?;
                let value_operand = if operator == "=" {
                    self.owned_operand(value, value_operand, *span)
                } else {
                    value_operand
                };
                let value = value_operand;
                let assigned_value = if operator == "=" {
                    Rvalue::Use(value)
                } else {
                    let Some(operator) = compound_operator(operator) else {
                        self.diagnostics.push(Diagnostic::error_with_code(
                            "NX2002",
                            format!("Zuweisungsoperator '{operator}' wird im MIR-Subset nicht unterstützt."),
                            *span,
                        ));
                        return None;
                    };
                    Rvalue::BinaryOp(operator, Operand::Copy(target_place.clone()), value)
                };
                self.push_statement(
                    StatementKind::Assign(target_place.clone(), assigned_value),
                    *span,
                );
                Some(Operand::Copy(target_place))
            }
            HirExpression::Unary {
                operator,
                operand,
                span,
            } => {
                if operator == "&" || operator == "&mut" {
                    self.unsupported_expression(
                        "Borrow-Ausdrücke müssen in dieser Stufe direkt einer lokalen Referenzbindung zugewiesen werden",
                        *span,
                    );
                    return None;
                }
                let value = self.lower_expression(operand)?;
                match operator.as_str() {
                    "+" => Some(value),
                    "*" => {
                        let HirExpression::Identifier(name, _) = operand.as_ref() else {
                            self.unsupported_expression(
                                "Dereferenzierung ist nur für lokale Referenzbindungen unterstützt",
                                *span,
                            );
                            return None;
                        };
                        let Some(local) = self.resolve_local(name) else {
                            self.diagnostics.push(Diagnostic::error_with_code(
                                "NX2002",
                                format!("Referenz '{name}' konnte beim Dereferenzieren nicht aufgelöst werden."),
                                *span,
                            ));
                            return None;
                        };
                        if !is_reference_type(&self.body.local_decls[local.0].type_name) {
                            self.unsupported_expression(
                                "Dereferenzierung erwartet eine lokale Referenz",
                                *span,
                            );
                            return None;
                        }
                        Some(Operand::Copy(Place::from_local(local).project_deref()))
                    }
                    "-" => {
                        let is_double = self.operand_type_name(&value).as_deref() == Some("double");
                        let destination = self.new_temp(if is_double { "double" } else { "int" });
                        let zero = if is_double {
                            Operand::ConstantFloat(0.0)
                        } else {
                            Operand::ConstantInt(0)
                        };
                        self.push_statement(
                            StatementKind::Assign(
                                Place::from_local(destination),
                                Rvalue::BinaryOp(BinaryOp::Sub, zero, value),
                            ),
                            *span,
                        );
                        Some(Operand::Copy(Place::from_local(destination)))
                    }
                    "!" => {
                        let destination = self.new_temp("bool");
                        self.push_statement(
                            StatementKind::Assign(
                                Place::from_local(destination),
                                Rvalue::BinaryOp(BinaryOp::Eq, value, Operand::ConstantBool(false)),
                            ),
                            *span,
                        );
                        Some(Operand::Copy(Place::from_local(destination)))
                    }
                    _ => {
                        self.unsupported_expression(operator, *span);
                        None
                    }
                }
            }
            HirExpression::Binary {
                left,
                operator,
                right,
                span,
            } => {
                if operator == "&&" || operator == "||" {
                    return self.lower_boolean_value(expression, *span);
                }
                let Some(operator) = binary_operator(operator) else {
                    self.unsupported_expression(operator, *span);
                    return None;
                };
                let left = self.lower_expression(left)?;
                let right = self.lower_expression(right)?;
                let type_name = if matches!(
                    operator,
                    BinaryOp::Eq
                        | BinaryOp::Ne
                        | BinaryOp::Lt
                        | BinaryOp::Le
                        | BinaryOp::Ge
                        | BinaryOp::Gt
                ) {
                    "bool"
                } else if operator == BinaryOp::Add && self.operand_is_string(&left) {
                    "string"
                } else if self.operand_type_name(&left).as_deref() == Some("double")
                    || self.operand_type_name(&right).as_deref() == Some("double")
                {
                    "double"
                } else {
                    "int"
                };
                let destination = self.new_temp(type_name);
                self.push_statement(
                    StatementKind::Assign(
                        Place::from_local(destination),
                        Rvalue::BinaryOp(operator, left, right),
                    ),
                    *span,
                );
                Some(Operand::Copy(Place::from_local(destination)))
            }
        }
    }

    fn lower_expression_into(
        &mut self,
        destination: LocalId,
        expression: &HirExpression,
        span: SourceSpan,
    ) -> bool {
        if let HirExpression::Unary {
            operator,
            operand,
            span: borrow_span,
        } = expression
            && (operator == "&" || operator == "&mut")
        {
            let borrowed = match operand.as_ref() {
                HirExpression::Identifier(name, _) => {
                    self.resolve_local(name).map(Place::from_local)
                }
                HirExpression::Unary {
                    operator, operand, ..
                } if operator == "*" => {
                    if let HirExpression::Identifier(name, _) = operand.as_ref() {
                        self.resolve_local(name)
                            .map(|local| Place::from_local(local).project_deref())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if !is_reference_type(&self.body.local_decls[destination.0].type_name) {
                self.diagnostics.push(Diagnostic::error_with_code(
                    "NX2002",
                    "Ein Borrow-Ausdruck benötigt ein Referenzziel.",
                    *borrow_span,
                ));
                return false;
            }
            let Some(borrowed) = borrowed else {
                self.diagnostics.push(Diagnostic::error_with_code(
                    "NX2002",
                    "Borrow-Ausdrücke dürfen nur lokale Werte oder dereferenzierte Referenzen referenzieren.",
                    *borrow_span,
                ));
                return false;
            };
            let kind = if operator == "&mut" {
                super::BorrowKind::Mut
            } else {
                super::BorrowKind::Shared
            };
            self.push_statement(
                StatementKind::Assign(Place::from_local(destination), Rvalue::Ref(kind, borrowed)),
                *borrow_span,
            );
            return true;
        }

        if let Some(value) = self.lower_expression(expression) {
            let value = self.owned_operand(expression, value, span);
            self.push_statement(
                StatementKind::Assign(Place::from_local(destination), Rvalue::Use(value)),
                span,
            );
            true
        } else {
            false
        }
    }

    fn lower_condition(
        &mut self,
        expression: &HirExpression,
        true_target: BasicBlockId,
        false_target: BasicBlockId,
        span: SourceSpan,
    ) -> bool {
        // Logical operators branch through the RHS only when short-circuit rules require it.
        if let HirExpression::Binary {
            left,
            operator,
            right,
            ..
        } = expression
        {
            if operator == "&&" {
                let right_block = self.new_block();
                if !self.lower_condition(left, right_block, false_target, left.span()) {
                    return false;
                }
                self.current_block = Some(right_block);
                return self.lower_condition(right, true_target, false_target, right.span());
            }
            if operator == "||" {
                let right_block = self.new_block();
                if !self.lower_condition(left, true_target, right_block, left.span()) {
                    return false;
                }
                self.current_block = Some(right_block);
                return self.lower_condition(right, true_target, false_target, right.span());
            }
        }

        let Some(discriminant) = self.lower_expression(expression) else {
            self.current_block = None;
            return false;
        };
        if self.current_block.is_none() {
            return false;
        }
        self.terminate(
            TerminatorKind::SwitchInt {
                discr: boolean_discriminant(discriminant),
                targets: vec![(1, true_target)],
                otherwise: false_target,
            },
            span,
        );
        self.current_block = None;
        true
    }

    fn lower_boolean_value(
        &mut self,
        expression: &HirExpression,
        span: SourceSpan,
    ) -> Option<Operand> {
        let destination = self.new_temp("bool");
        let true_block = self.new_block();
        let false_block = self.new_block();
        let join_block = self.new_block();
        if !self.lower_condition(expression, true_block, false_block, span) {
            return None;
        }

        self.current_block = Some(true_block);
        self.push_statement(
            StatementKind::Assign(
                Place::from_local(destination),
                Rvalue::Use(Operand::ConstantBool(true)),
            ),
            span,
        );
        self.terminate(TerminatorKind::Goto { target: join_block }, span);

        self.current_block = Some(false_block);
        self.push_statement(
            StatementKind::Assign(
                Place::from_local(destination),
                Rvalue::Use(Operand::ConstantBool(false)),
            ),
            span,
        );
        self.terminate(TerminatorKind::Goto { target: join_block }, span);
        self.current_block = Some(join_block);
        Some(Operand::Copy(Place::from_local(destination)))
    }

    fn lower_conditional_expression(
        &mut self,
        condition: &HirExpression,
        when_true: &HirExpression,
        when_false: &HirExpression,
        span: SourceSpan,
    ) -> Option<Operand> {
        let type_name = self.expression_type(when_true)?;
        if self.expression_type(when_false).as_deref() != Some(type_name.as_str()) {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX2002",
                "Die Zweige eines bedingten MIR-Ausdrucks müssen denselben skalaren Typ besitzen.",
                span,
            ));
            return None;
        }
        let destination = self.new_temp(&type_name);
        let true_block = self.new_block();
        let false_block = self.new_block();
        let join_block = self.new_block();
        if !self.lower_condition(condition, true_block, false_block, span) {
            return None;
        }

        self.current_block = Some(true_block);
        let value = self.lower_expression(when_true)?;
        let value = self.owned_operand(when_true, value, when_true.span());
        self.push_statement(
            StatementKind::Assign(Place::from_local(destination), Rvalue::Use(value)),
            when_true.span(),
        );
        self.terminate(TerminatorKind::Goto { target: join_block }, span);

        self.current_block = Some(false_block);
        let value = self.lower_expression(when_false)?;
        let value = self.owned_operand(when_false, value, when_false.span());
        self.push_statement(
            StatementKind::Assign(Place::from_local(destination), Rvalue::Use(value)),
            when_false.span(),
        );
        self.terminate(TerminatorKind::Goto { target: join_block }, span);
        self.current_block = Some(join_block);
        Some(Operand::Copy(Place::from_local(destination)))
    }

    fn expression_type(&self, expression: &HirExpression) -> Option<String> {
        match expression {
            HirExpression::Identifier(name, _) => self
                .resolve_local(name)
                .map(|local| self.body.local_decls[local.0].type_name.clone()),
            HirExpression::Field { type_name, .. }
            | HirExpression::FieldAssignment { type_name, .. } => Some(type_name.clone()),
            HirExpression::New { class, .. } => Some(class.clone()),
            HirExpression::Integer(_, _) => Some("int".to_owned()),
            HirExpression::Float(_, _) => Some("double".to_owned()),
            HirExpression::String(_, _) => Some("string".to_owned()),
            HirExpression::Boolean(_, _) => Some("bool".to_owned()),
            HirExpression::Call { return_type, .. } => Some(return_type.clone()),
            HirExpression::Unary {
                operator, operand, ..
            } => {
                if operator == "!" {
                    Some("bool".to_owned())
                } else if operator == "*" {
                    self.expression_type(operand).and_then(|type_name| {
                        type_name
                            .strip_prefix("&mut ")
                            .or_else(|| type_name.strip_prefix('&'))
                            .map(str::to_owned)
                    })
                } else {
                    self.expression_type(operand)
                }
            }
            HirExpression::Conditional {
                when_true,
                when_false,
                ..
            } => {
                let true_type = self.expression_type(when_true)?;
                (self.expression_type(when_false).as_deref() == Some(true_type.as_str()))
                    .then_some(true_type)
            }
            HirExpression::Assignment { target, .. } => self
                .resolve_local(target)
                .map(|local| self.body.local_decls[local.0].type_name.clone()),
            HirExpression::Binary {
                operator,
                left,
                right,
                ..
            } => Some(
                if matches!(
                    operator.as_str(),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
                ) {
                    "bool"
                } else if operator == "+" && self.expression_type(left).as_deref() == Some("string")
                {
                    "string"
                } else if self.expression_type(left).as_deref() == Some("double")
                    || self.expression_type(right).as_deref() == Some("double")
                {
                    "double"
                } else {
                    "int"
                }
                .to_owned(),
            ),
        }
    }

    fn operand_type_name(&self, operand: &Operand) -> Option<String> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
                Some(self.body.local_decls[place.local.0].type_name.clone())
            }
            Operand::ConstantFloat(_) => Some("double".to_owned()),
            _ => None,
        }
    }

    fn operand_is_string(&self, operand: &Operand) -> bool {
        match operand {
            Operand::ConstantString(_) => true,
            Operand::Copy(place) | Operand::Move(place) => {
                self.body.local_decls[place.local.0].type_name == "string"
            }
            _ => false,
        }
    }

    /// Marks `string` identifiers in ownership-transferring positions (initializer,
    /// assignment, return value, call argument) as moved, mirroring the semantic pass.
    fn owned_operand(
        &mut self,
        expression: &HirExpression,
        operand: Operand,
        span: SourceSpan,
    ) -> Operand {
        let (HirExpression::Identifier(..), Operand::Copy(place)) = (expression, &operand) else {
            return operand;
        };
        let local = &self.body.local_decls[place.local.0];
        if local.type_name != "string" || !place.projection.is_empty() {
            return operand;
        }
        if local.global_symbol.is_some() {
            if !local.is_mut {
                return operand;
            }
            // A `mut string` global keeps its buffer, so ownership transfer copies it.
            let copy = self.new_temp("string");
            self.push_statement(
                StatementKind::Assign(
                    Place::from_local(copy),
                    Rvalue::BinaryOp(
                        BinaryOp::Add,
                        operand,
                        Operand::ConstantString(String::new()),
                    ),
                ),
                span,
            );
            return Operand::Copy(Place::from_local(copy));
        }
        Operand::Move(place.clone())
    }

    fn is_block_reachable(&self, target: BasicBlockId) -> bool {
        let mut reachable = HashSet::new();
        let mut pending = vec![BasicBlockId(0)];
        while let Some(block_id) = pending.pop() {
            if block_id == target {
                return true;
            }
            if !reachable.insert(block_id) {
                continue;
            }
            let Some(terminator) = self.body.basic_blocks[block_id.0].terminator.as_ref() else {
                continue;
            };
            match &terminator.kind {
                TerminatorKind::SwitchInt {
                    discr: Operand::ConstantInt(value),
                    targets,
                    otherwise,
                } => pending.push(
                    targets
                        .iter()
                        .find(|(candidate, _)| candidate == value)
                        .map_or(*otherwise, |(_, target)| *target),
                ),
                TerminatorKind::SwitchInt {
                    targets, otherwise, ..
                } => {
                    pending.extend(targets.iter().map(|(_, target)| *target));
                    pending.push(*otherwise);
                }
                TerminatorKind::Goto { target } => pending.push(*target),
                TerminatorKind::Drop { target, unwind, .. } => {
                    pending.push(*target);
                    pending.extend(unwind.iter().copied());
                }
                TerminatorKind::Call {
                    target, cleanup, ..
                }
                | TerminatorKind::Assert {
                    target, cleanup, ..
                } => {
                    pending.push(*target);
                    pending.extend(cleanup.iter().copied());
                }
                TerminatorKind::Return | TerminatorKind::Unreachable => {}
            }
        }
        false
    }

    fn finish(mut self) -> Result<MirBody, Vec<Diagnostic>> {
        if self.current_block.is_some() {
            if self.function.return_type == "void" {
                let span = self.function.body.span;
                self.terminate(TerminatorKind::Return, span);
            } else {
                self.diagnostics.push(Diagnostic::error_with_code(
                    "NX2002",
                    "Nicht-void-Funktion erreicht beim MIR-Lowering das Ende ohne return.",
                    self.function.body.span,
                ));
            }
        }
        if self.diagnostics.is_empty() {
            Ok(self.body)
        } else {
            Err(self.diagnostics)
        }
    }

    fn allocate_local(
        &mut self,
        name: &str,
        type_name: &str,
        is_mut: bool,
        is_user_variable: bool,
    ) -> LocalId {
        let local = LocalId(self.body.local_decls.len());
        let field_types = type_name
            .strip_prefix("class ")
            .and_then(|class| self.classes.get(class))
            .map(|fields| fields.iter().map(|(_, ty)| ty.clone()).collect())
            .unwrap_or_default();
        self.body.local_decls.push(LocalDecl {
            name: Some(name.to_owned()),
            type_name: type_name.to_owned(),
            field_types,
            is_mut,
            is_user_variable,
            global_symbol: None,
        });
        if let Some(locals) = self.scope_locals.last_mut() {
            locals.push(local);
        }
        local
    }

    fn end_scope(&mut self) {
        if self.scopes.pop().is_none() {
            return;
        }
        let locals = self.scope_locals.pop().unwrap_or_default();
        if self.current_block.is_some() {
            for local in locals.into_iter().rev() {
                self.push_statement(StatementKind::StorageDead(local), self.function.body.span);
            }
        }
    }

    fn new_temp(&mut self, type_name: &str) -> LocalId {
        let local = LocalId(self.body.local_decls.len());
        let field_types = type_name
            .strip_prefix("class ")
            .and_then(|class| self.classes.get(class))
            .map(|fields| fields.iter().map(|(_, ty)| ty.clone()).collect())
            .unwrap_or_default();
        self.body.local_decls.push(LocalDecl {
            name: None,
            type_name: type_name.to_owned(),
            field_types,
            is_mut: true,
            is_user_variable: false,
            global_symbol: None,
        });
        local
    }

    fn new_block(&mut self) -> BasicBlockId {
        let block = BasicBlockId(self.body.basic_blocks.len());
        self.body.basic_blocks.push(BasicBlockData::default());
        block
    }

    fn resolve_local(&self, name: &str) -> Option<LocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn push_statement(&mut self, kind: StatementKind, span: SourceSpan) {
        if let Some(block) = self
            .current_block
            .and_then(|id| self.body.basic_blocks.get_mut(id.0))
        {
            block.statements.push(Statement {
                kind,
                source_span: source_range(span),
            });
        }
    }

    fn terminate(&mut self, kind: TerminatorKind, span: SourceSpan) {
        if let Some(block) = self
            .current_block
            .and_then(|id| self.body.basic_blocks.get_mut(id.0))
        {
            block.terminator = Some(Terminator {
                kind,
                source_span: source_range(span),
            });
        }
    }

    fn unsupported_expression(&mut self, expression: &str, span: SourceSpan) {
        self.diagnostics.push(Diagnostic::error_with_code(
            "NX2002",
            format!("Operator '{expression}' wird im aktuellen MIR-Subset nicht unterstützt."),
            span,
        ));
    }
}

fn source_range(span: SourceSpan) -> (usize, usize) {
    (span.start.offset, span.end.offset)
}

fn boolean_discriminant(operand: Operand) -> Operand {
    match operand {
        Operand::ConstantBool(value) => Operand::ConstantInt(i64::from(value)),
        operand => operand,
    }
}

fn is_mir_scalar(type_name: &str) -> bool {
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
            | "IntList"
            | "StringList"
    )
}

fn is_reference_type(type_name: &str) -> bool {
    type_name.starts_with('&')
}

fn is_mir_supported_type(type_name: &str) -> bool {
    type_name == "string"
        || type_name.starts_with("class ")
        || is_mir_scalar(type_name)
        || (is_reference_type(type_name)
            && is_mir_scalar(
                type_name
                    .trim_start_matches("&mut ")
                    .trim_start_matches('&'),
            ))
}

fn console_runtime_symbol(callee: &str) -> Option<&'static str> {
    match callee {
        "Console.write" => Some("__nortex_console_write"),
        "Console.writeLine" => Some("__nortex_console_write_line"),
        _ => None,
    }
}

fn object_type_name(class: &str) -> String {
    format!("class {class}")
}

fn zero_operand(type_name: &str) -> Operand {
    match type_name {
        "bool" => Operand::ConstantBool(false),
        "double" => Operand::ConstantFloat(0.0),
        _ => Operand::ConstantInt(0),
    }
}

fn is_copy_type(type_name: &str) -> bool {
    type_name.starts_with("class ")
        || type_name == "string"
        || is_mir_scalar(type_name)
        || (is_reference_type(type_name) && !type_name.starts_with("&mut "))
}

fn binary_operator(operator: &str) -> Option<BinaryOp> {
    Some(match operator {
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "/" => BinaryOp::Div,
        "%" => BinaryOp::Rem,
        "^" => BinaryOp::BitXor,
        "&" => BinaryOp::BitAnd,
        "|" => BinaryOp::BitOr,
        "<<" => BinaryOp::Shl,
        ">>" => BinaryOp::Shr,
        "==" => BinaryOp::Eq,
        "<" => BinaryOp::Lt,
        "<=" => BinaryOp::Le,
        "!=" => BinaryOp::Ne,
        ">=" => BinaryOp::Ge,
        ">" => BinaryOp::Gt,
        _ => return None,
    })
}

fn compound_operator(operator: &str) -> Option<BinaryOp> {
    let operator = operator.strip_suffix('=')?;
    binary_operator(operator)
}

fn parse_integer(value: &str) -> Option<i64> {
    let cleaned = value.replace('_', "").to_ascii_lowercase();
    let literal = [
        "i128", "u128", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8", "long", "ulong",
        "int", "uint", "short", "ushort", "byte", "sbyte",
    ]
    .iter()
    .find_map(|suffix| cleaned.strip_suffix(suffix))
    .unwrap_or(&cleaned);
    let value = if let Some(hex) = literal.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(binary) = literal.strip_prefix("0b") {
        i64::from_str_radix(binary, 2).ok()?
    } else {
        literal.parse().ok()?
    };
    Some(value)
}

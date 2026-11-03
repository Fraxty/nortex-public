use super::*;

#[derive(Clone, Copy)]
pub(super) struct FunctionValidationContext<'a> {
    pub(super) known_types: &'a HashMap<String, KnownType>,
    pub(super) enclosing_generics: &'a HashSet<String>,
    pub(super) fields: &'a HashSet<String>,
    pub(super) immutable_fields: &'a HashSet<String>,
    pub(super) global_values: &'a HashSet<String>,
    pub(super) immutable_globals: &'a HashSet<String>,
    pub(super) class_declarations: &'a HashMap<&'a str, &'a ClassDeclaration>,
}

#[derive(Clone, Copy)]
pub(super) struct GlobalValueContext<'a> {
    pub(super) names: &'a HashSet<String>,
    pub(super) immutable: &'a HashSet<String>,
}

pub(super) fn validate_function(
    function: &FunctionDeclaration,
    owner_name: Option<&str>,
    context: FunctionValidationContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A constructor is identified by the enclosing class name, not by the
    // accidental combination of PascalCase and a void return type. Ordinary
    // PascalCase methods still receive the normal naming diagnostic.
    let is_c_abi = function
        .modifiers
        .iter()
        .any(|modifier| modifier == "extern");
    if owner_name != Some(function.name.as_str()) && !is_c_abi {
        validate_value_name(&function.name, function.span, diagnostics);
    }
    let mut generics = context.enclosing_generics.clone();
    for generic in &function.generic_parameters {
        if !generics.insert(generic.clone()) {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1001",
                format!("Generischer Parameter '{generic}' ist mehrfach definiert."),
                function.span,
            ));
        }
    }
    for generic in &function.generic_parameters {
        validate_identifier(generic, "Generischer Parameter", function.span, diagnostics);
    }
    validate_type(
        &function.return_type,
        context.known_types,
        &generics,
        diagnostics,
    );
    let has_raw_pointer = contains_raw_pointer(&function.return_type)
        || function
            .parameters
            .iter()
            .any(|parameter| contains_raw_pointer(&parameter.type_ref));
    if has_raw_pointer
        && !function
            .modifiers
            .iter()
            .any(|modifier| modifier == "unsafe")
    {
        diagnostics.push(Diagnostic::error(
            "Funktionen mit Raw-Pointer-Typen in Parametern oder Rückgabewerten müssen als unsafe deklariert werden.",
            function.span));
    }

    let mut parameter_names = HashSet::new();
    for parameter in &function.parameters {
        validate_type(
            &parameter.type_ref,
            context.known_types,
            &generics,
            diagnostics,
        );
        if !parameter_names.insert(parameter.name.clone()) {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1001",
                format!("Doppelter Parameter '{}'.", parameter.name),
                parameter.span,
            ));
        }
        validate_value_name(&parameter.name, parameter.span, diagnostics);
    }

    if let Some(body) = &function.body {
        let mut immutable: HashSet<String> = context
            .immutable_fields
            .iter()
            .map(|field| format!("field:{field}"))
            .collect();
        immutable.extend(context.immutable_globals.iter().cloned());
        let mut visible = context.fields.clone();
        visible.extend(context.global_values.iter().cloned());
        for parameter in &function.parameters {
            // An &mut parameter cannot be rebound, but its referent may be
            // mutated. Direct rebinding is checked in the dedicated pass.
            if parameter.type_ref.reference_kind != ReferenceKind::MutableBorrow {
                immutable.insert(parameter.name.clone());
            }
            visible.insert(parameter.name.clone());
        }
        analyze_block(body, &mut visible, &mut immutable, diagnostics);
        let effective_return_type = if function
            .modifiers
            .iter()
            .any(|modifier| modifier == "async")
            && function.return_type.name == "Task"
            && function.return_type.generic_arguments.len() == 1
        {
            &function.return_type.generic_arguments[0]
        } else {
            &function.return_type
        };
        validate_return_literals(body, effective_return_type, diagnostics);
        if effective_return_type.name != "void" && effective_return_type.name != "!" {
            let enum_variants = context
                .class_declarations
                .values()
                .filter(|class| class.kind == TypeDeclarationKind::Enum)
                .map(|class| {
                    (
                        class.name.clone(),
                        class
                            .members
                            .iter()
                            .filter_map(|member| match member {
                                MemberDeclaration::Variant(variant) => {
                                    Some(variant.name.as_str().to_owned())
                                }
                                _ => None,
                            })
                            .collect::<HashSet<_>>(),
                    )
                })
                .filter(|(_, variants)| !variants.is_empty())
                .collect::<HashMap<_, _>>();
            let parameter_types = function
                .parameters
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.type_ref.name.clone()))
                .collect::<HashMap<_, _>>();
            let flow = analyze_return_flow(body, &enum_variants, &parameter_types);
            if flow.falls_through {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1103",
                    format!(
                        "Nicht jeder Kontrollfluss in '{}' liefert einen Rückgabewert.",
                        function.name
                    ),
                    function.span,
                ));
            }
        }
        validate_control_flow(body, 0, 0, diagnostics);
        validate_borrowed_parameter_mutation(function, body, diagnostics);
    }
}

#[derive(Clone, Copy, Default)]
struct ReturnFlow {
    falls_through: bool,
    returns: bool,
    throws: bool,
    breaks: bool,
    continues: bool,
}

impl ReturnFlow {
    fn terminal() -> Self {
        Self {
            falls_through: false,
            ..Self::default()
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            falls_through: self.falls_through || other.falls_through,
            returns: self.returns || other.returns,
            throws: self.throws || other.throws,
            breaks: self.breaks || other.breaks,
            continues: self.continues || other.continues,
        }
    }

    fn then(self, next: Self) -> Self {
        if self.falls_through {
            Self {
                falls_through: next.falls_through,
                returns: self.returns || next.returns,
                throws: self.throws || next.throws,
                breaks: self.breaks || next.breaks,
                continues: self.continues || next.continues,
            }
        } else {
            self
        }
    }
}

fn analyze_return_flow(
    block: &BlockStatement,
    enum_variants: &HashMap<String, HashSet<String>>,
    variable_types: &HashMap<String, String>,
) -> ReturnFlow {
    block.statements.iter().fold(
        ReturnFlow {
            falls_through: true,
            ..ReturnFlow::default()
        },
        |flow, statement| {
            flow.then(analyze_statement_return_flow(
                statement,
                enum_variants,
                variable_types,
            ))
        },
    )
}

fn analyze_statement_return_flow(
    statement: &Statement,
    enum_variants: &HashMap<String, HashSet<String>>,
    variable_types: &HashMap<String, String>,
) -> ReturnFlow {
    match statement {
        Statement::Block(block) => analyze_return_flow(block, enum_variants, variable_types),
        Statement::Return(_, _) => ReturnFlow {
            returns: true,
            ..ReturnFlow::terminal()
        },
        Statement::Throw(_, _) => ReturnFlow {
            throws: true,
            ..ReturnFlow::terminal()
        },
        Statement::Break(_) => ReturnFlow {
            breaks: true,
            ..ReturnFlow::terminal()
        },
        Statement::Continue(_) => ReturnFlow {
            continues: true,
            ..ReturnFlow::terminal()
        },
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            if let Some(value) = constant_boolean(condition) {
                return if value {
                    analyze_statement_return_flow(then_branch, enum_variants, variable_types)
                } else {
                    else_branch.as_deref().map_or_else(
                        || ReturnFlow {
                            falls_through: true,
                            ..ReturnFlow::default()
                        },
                        |other| analyze_statement_return_flow(other, enum_variants, variable_types),
                    )
                };
            }
            let then_flow =
                analyze_statement_return_flow(then_branch, enum_variants, variable_types);
            else_branch.as_deref().map_or(
                ReturnFlow {
                    falls_through: true,
                    ..then_flow
                },
                |other| {
                    then_flow.merge(analyze_statement_return_flow(
                        other,
                        enum_variants,
                        variable_types,
                    ))
                },
            )
        }
        Statement::While {
            condition, body, ..
        } => {
            if constant_boolean(condition) == Some(false) {
                return ReturnFlow {
                    falls_through: true,
                    ..ReturnFlow::default()
                };
            }
            let body_flow = analyze_statement_return_flow(body, enum_variants, variable_types);
            let always_true = constant_boolean(condition) == Some(true);
            ReturnFlow {
                falls_through: !always_true || body_flow.breaks,
                returns: body_flow.returns,
                throws: body_flow.throws,
                ..ReturnFlow::default()
            }
        }
        Statement::For {
            condition, body, ..
        } => {
            if condition.as_ref().and_then(constant_boolean) == Some(false) {
                return ReturnFlow {
                    falls_through: true,
                    ..ReturnFlow::default()
                };
            }
            let body_flow = analyze_statement_return_flow(body, enum_variants, variable_types);
            let always_true = condition
                .as_ref()
                .is_none_or(|condition| constant_boolean(condition) == Some(true));
            ReturnFlow {
                falls_through: !always_true || body_flow.breaks,
                returns: body_flow.returns,
                throws: body_flow.throws,
                ..ReturnFlow::default()
            }
        }

        Statement::ForEach { body, .. } => {
            let body_flow = analyze_statement_return_flow(body, enum_variants, variable_types);
            ReturnFlow {
                falls_through: true,
                returns: body_flow.returns,
                throws: body_flow.throws,
                ..ReturnFlow::default()
            }
        }
        Statement::Switch { cases, .. } => {
            let has_default = cases.iter().any(|case| case.value.is_none());
            let mut flow = ReturnFlow {
                falls_through: !has_default,
                ..ReturnFlow::default()
            };
            for case in cases {
                let case_flow = case.statements.iter().fold(
                    ReturnFlow {
                        falls_through: true,
                        ..ReturnFlow::default()
                    },
                    |flow, statement| {
                        flow.then(analyze_statement_return_flow(
                            statement,
                            enum_variants,
                            variable_types,
                        ))
                    },
                );
                flow = flow.merge(ReturnFlow {
                    falls_through: case_flow.falls_through || case_flow.breaks,
                    breaks: false,
                    continues: case_flow.continues,
                    ..case_flow
                });
            }
            flow
        }
        Statement::Match { value, arms, .. } => {
            let matched_enum = match value {
                Expression::Identifier(name, _) => variable_types.get(name),
                _ => None,
            };
            let covered_variants = arms
                .iter()
                .map(|arm| arm.variant.rsplit('.').next().unwrap_or(&arm.variant))
                .collect::<HashSet<_>>();
            let exhaustive = arms.iter().any(|arm| arm.variant == "_")
                || matched_enum
                    .and_then(|enum_name| enum_variants.get(enum_name))
                    .is_some_and(|variants| {
                        variants
                            .iter()
                            .all(|variant| covered_variants.contains(variant.as_str()))
                    });
            let mut flow = ReturnFlow {
                falls_through: !exhaustive,
                ..ReturnFlow::default()
            };
            for arm in arms {
                flow = flow.merge(analyze_return_flow(
                    &arm.body,
                    enum_variants,
                    variable_types,
                ));
            }
            flow
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            let mut flow = analyze_return_flow(body, enum_variants, variable_types);
            for catch in catches {
                flow = flow.merge(analyze_return_flow(
                    &catch.body,
                    enum_variants,
                    variable_types,
                ));
            }
            if let Some(finally_block) = finally_block {
                let finally_flow =
                    analyze_return_flow(finally_block, enum_variants, variable_types);
                if !finally_flow.falls_through {
                    return finally_flow;
                }
                flow.returns |= finally_flow.returns;
                flow.throws |= finally_flow.throws;
                flow.breaks |= finally_flow.breaks;
                flow.continues |= finally_flow.continues;
            }
            flow
        }
        Statement::Variable(_) | Statement::Expression(_, _) | Statement::Empty(_) => ReturnFlow {
            falls_through: true,
            ..ReturnFlow::default()
        },
    }
}

pub(super) fn constant_boolean(expression: &Expression) -> Option<bool> {
    match expression {
        Expression::Boolean(value, _) => Some(*value),
        Expression::Group(inner, _) => constant_boolean(inner),
        Expression::Unary {
            operator, operand, ..
        } if operator == "!" => Some(!constant_boolean(operand)?),
        Expression::Binary {
            left,
            operator,
            right,
            ..
        } if operator == "&&" => match (constant_boolean(left), constant_boolean(right)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Expression::Binary {
            left,
            operator,
            right,
            ..
        } if operator == "||" => match (constant_boolean(left), constant_boolean(right)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => match constant_boolean(condition) {
            Some(true) => constant_boolean(when_true),
            Some(false) => constant_boolean(when_false),
            None => None,
        },
        _ => None,
    }
}

pub(super) fn literal_category(expression: &Expression) -> Option<&'static str> {
    match expression {
        Expression::Group(inner, _) => literal_category(inner),
        Expression::Integer(_, _) | Expression::Float(_, _) => Some("numeric"),
        Expression::Boolean(_, _) => Some("bool"),
        Expression::String(_, _) => Some("string"),
        Expression::Character(_, _) => Some("char"),
        _ => None,
    }
}

pub(super) fn type_category(type_ref: &TypeReference) -> Option<&'static str> {
    if type_ref.reference_kind != ReferenceKind::Value
        || type_ref.array_depth > 0
        || type_ref.fixed_array_length.is_some()
    {
        return None;
    }
    match type_ref.name.as_str() {
        "bool" => Some("bool"),
        "string" => Some("string"),
        "char" => Some("char"),
        "sbyte" | "short" | "int" | "long" | "i128" | "byte" | "ushort" | "uint" | "ulong"
        | "u128" | "nint" | "nuint" | "isize" | "usize" | "float" | "double" | "decimal" => {
            Some("numeric")
        }
        "void" => Some("void"),
        _ => None,
    }
}

pub(super) fn validate_literal_against_type(
    expected: &TypeReference,
    value: &Expression,
    code: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(actual_category) = literal_category(value) else {
        return;
    };
    let expected_category = type_category(expected);
    let compatible = expected_category == Some(actual_category)
        || (expected_category == Some("numeric") && actual_category == "numeric");
    if !compatible {
        diagnostics.push(Diagnostic::error_with_code(
            code,
            format!("Der Wertliteral-Typ '{actual_category}' ist nicht mit dem erwarteten Typ '{}' kompatibel.", expected.name),
            value.span(),
        ).with_help("Passe den deklarierten Typ oder das Literal an. NORTEX führt keine implizite Umwandlung zwischen unterschiedlichen primitiven Kategorien durch."));
    }
}

pub(super) fn validate_variable_literal_type(
    variable: &VariableDeclaration,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expected) = &variable.type_ref else {
        return;
    };
    if expected.name == "void" || expected.name == "!" {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1102",
            format!(
                "Der Typ '{}' kann nicht als Variablentyp verwendet werden.",
                expected.name
            ),
            expected.span,
        ));
        return;
    }
    if let Some(value) = &variable.initializer {
        validate_literal_against_type(expected, value, "NX1102", diagnostics);
    }
}

pub(super) fn validate_return_literals(
    block: &BlockStatement,
    expected: &TypeReference,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn visit(statement: &Statement, expected: &TypeReference, diagnostics: &mut Vec<Diagnostic>) {
        match statement {
            Statement::Return(Some(value), _) => {
                validate_literal_against_type(expected, value, "NX1102", diagnostics)
            }
            Statement::Return(None, span) if expected.name != "void" => {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1103",
                    "Eine Funktion mit Rückgabetyp benötigt in diesem return einen Rückgabewert.",
                    *span,
                ))
            }
            Statement::Block(child) => validate_return_literals(child, expected, diagnostics),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                visit(then_branch, expected, diagnostics);
                if let Some(other) = else_branch {
                    visit(other, expected, diagnostics);
                }
            }
            Statement::While { body, .. }
            | Statement::For { body, .. }
            | Statement::ForEach { body, .. } => visit(body, expected, diagnostics),
            Statement::Switch { cases, .. } => {
                for case in cases {
                    for child in &case.statements {
                        visit(child, expected, diagnostics);
                    }
                }
            }
            Statement::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                validate_return_literals(body, expected, diagnostics);
                for clause in catches {
                    validate_return_literals(&clause.body, expected, diagnostics);
                }
                if let Some(finally_body) = finally_block {
                    validate_return_literals(finally_body, expected, diagnostics);
                }
            }
            Statement::Match { arms, .. } => {
                for arm in arms {
                    validate_return_literals(&arm.body, expected, diagnostics);
                }
            }
            _ => {}
        }
    }

    for statement in &block.statements {
        visit(statement, expected, diagnostics);
    }
}

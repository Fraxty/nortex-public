use super::*;

pub(super) fn is_builtin_type_name(name: &str) -> bool {
    crate::type_system::BUILTIN_TYPE_NAMES.contains(&name)
        || matches!(
            name,
            "object"
                | "dynamic"
                | "Option"
                | "Result"
                | "Task"
                | "Future"
                | "Box"
                | "Rc"
                | "Arc"
                | "Cell"
                | "RefCell"
                | "Mutex"
                | "RwLock"
                | "Span"
                | "Memory"
        )
}

pub(super) fn validate_expression_names(
    expression: &Expression,
    scope: &HashSet<String>,
    globals: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expression::Identifier(name, span) => {
            if !scope.contains(name) && !globals.contains(name) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1004",
                    format!("Unbekannter Bezeichner '{name}'. Der Wert wurde weder deklariert noch importiert.") ,
                    *span,
                ).with_help("Prüfe die Schreibweise oder deklariere den Wert vor seiner Verwendung."));
            }
        }
        Expression::Unary { operand, .. } | Expression::Group(operand, _) => {
            validate_expression_names(operand, scope, globals, diagnostics);
        }
        Expression::Binary { left, right, .. } => {
            validate_expression_names(left, scope, globals, diagnostics);
            validate_expression_names(right, scope, globals, diagnostics);
        }
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            validate_expression_names(condition, scope, globals, diagnostics);
            validate_expression_names(when_true, scope, globals, diagnostics);
            validate_expression_names(when_false, scope, globals, diagnostics);
        }
        Expression::Assignment { target, value, .. } => {
            validate_expression_names(target, scope, globals, diagnostics);
            validate_expression_names(value, scope, globals, diagnostics);
        }
        Expression::Call {
            callee, arguments, ..
        } => {
            validate_expression_names(callee, scope, globals, diagnostics);
            for argument in arguments {
                validate_expression_names(argument, scope, globals, diagnostics);
            }
        }
        Expression::ObjectCreation {
            type_ref,
            arguments,
            ..
        } => {
            if !globals.contains(&type_ref.name) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1100",
                    format!("Unbekannter Typ '{}' in Konstruktoraufruf.", type_ref.name),
                    type_ref.span,
                ));
            }
            for argument_type in &type_ref.generic_arguments {
                if !globals.contains(&argument_type.name)
                    && !is_builtin_type_name(&argument_type.name)
                {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1100",
                        format!("Unbekannter generischer Typ '{}'.", argument_type.name),
                        argument_type.span,
                    ));
                }
            }
            for argument in arguments {
                validate_expression_names(argument, scope, globals, diagnostics);
            }
        }
        Expression::Member { object, .. } => {
            validate_expression_names(object, scope, globals, diagnostics)
        }
        Expression::Index { object, index, .. } => {
            validate_expression_names(object, scope, globals, diagnostics);
            validate_expression_names(index, scope, globals, diagnostics);
        }
        Expression::Integer(..)
        | Expression::Float(..)
        | Expression::String(..)
        | Expression::Character(..)
        | Expression::Boolean(..)
        | Expression::Null(..) => {}
    }
}

pub(super) fn analyze_match_arms(
    arms: &[MatchArm],
    visible: &HashSet<String>,
    immutable: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_variants = HashSet::new();
    let mut wildcard_seen = false;
    for (index, arm) in arms.iter().enumerate() {
        if arm.variant == "_" {
            if wildcard_seen {
                diagnostics.push(Diagnostic::error(
                    "Ein match darf höchstens einen '_' -Fallback-Arm besitzen.",
                    arm.span,
                ));
            }
            wildcard_seen = true;
            if index + 1 != arms.len() {
                diagnostics.push(Diagnostic::warning(
                    "Der '_' -Fallback-Arm sollte als letzter match-Arm stehen.",
                    arm.span,
                ));
            }
        } else if !seen_variants.insert(arm.variant.clone()) {
            diagnostics.push(Diagnostic::error(
                format!(
                    "Die Enum-Variante '{}' wird im match mehrfach behandelt.",
                    arm.variant
                ),
                arm.span,
            ));
        }

        let mut arm_visible = visible.clone();
        let mut arm_immutable = immutable.clone();
        if let Some(binding) = &arm.binding {
            if !arm_visible.insert(binding.clone()) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1001",
                    format!(
                        "Match-Bindung '{binding}' ist bereits im Gültigkeitsbereich definiert."
                    ),
                    arm.span,
                ));
            }
            arm_immutable.insert(binding.clone());
            validate_value_name(binding, arm.span, diagnostics);
        }
        analyze_block(&arm.body, &mut arm_visible, &mut arm_immutable, diagnostics);
    }
}

pub(super) fn diagnose_immutable_member_mutation(
    target: &Expression,
    immutable: &HashSet<String>,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(root) = root_identifier(target).filter(|root| immutable.contains(*root)) {
        diagnostics.push(Diagnostic::error_with_code(
        "NX1201",
        format!("Die Veränderung über ein Member oder einen Index erfordert einen exklusiven &mut-Borrow; '{root}' ist hier nur unveränderlich ausgeliehen."),
        span,
    ))
    }
}

pub(super) fn analyze_expression(
    expression: &Expression,
    immutable: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expression::Assignment { target, value, span, .. } => {
            if let Some(name) = match target.as_ref() {
                Expression::Identifier(name, _)
                    if immutable.contains(name) || immutable.contains(&format!("field:{name}")) => Some(name.as_str()),
                _ => None,
            } {
                diagnostics.push(Diagnostic::error_with_code("NX1200", format!("Der unveränderliche Wert '{name}' darf nicht neu zugewiesen werden."), *span));
            } else if matches!(target.as_ref(), Expression::Member { .. } | Expression::Index { .. }) {
                diagnose_immutable_member_mutation(target, immutable, *span, diagnostics);
                let immutable_this_field = match target.as_ref() {
                    Expression::Member { object, name, .. }
                        if matches!(object.as_ref(), Expression::Identifier(this_name, _) if this_name == "this")
                            && immutable.contains(&format!("field:{name}")) => Some(name),
                    _ => None,
                };
                if let Some(name) = immutable_this_field {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1200", format!("Das unveränderliche Feld '{name}' darf nicht verändert werden."), *span));
                }
            }
            analyze_expression(target, immutable, diagnostics);
            analyze_expression(value, immutable, diagnostics);
        }
        Expression::Unary { operator, operand, span } => {
            let is_increment = matches!(operator.as_str(), "++" | "--" | "post++" | "post--");
            if operator == "&mut"
                && let Some(name) = root_identifier(operand).filter(|name| immutable.contains(*name))
            {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1201",
                    format!("Der unveränderliche Wert '{name}' kann nicht exklusiv ausgeliehen werden; deklariere ihn mit 'mut'."),
                    *span,
                ));
            }
            if let Some(name) = identifier_in_set_when(operand, immutable, is_increment) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1200",
                    format!(
                        "Der unveränderliche Wert '{name}' darf nicht inkrementiert oder dekrementiert werden."
                    ),
                    *span,
                ));
            } else if let Some(root) = root_identifier(operand).filter(|root| {
                is_increment
                    && matches!(operand.as_ref(), Expression::Member { .. } | Expression::Index { .. })
                    && immutable.contains(*root)
            }) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1201", format!("Die Veränderung über ein Member oder einen Index erfordert einen exklusiven &mut-Borrow; '{root}' ist hier nur unveränderlich ausgeliehen."), *span));
            }
            analyze_expression(operand, immutable, diagnostics);
        }
        Expression::Group(operand, _) => analyze_expression(operand, immutable, diagnostics),
        Expression::Conditional { condition, when_true, when_false, .. } => {
            analyze_expression(condition, immutable, diagnostics);
            analyze_expression(when_true, immutable, diagnostics);
            analyze_expression(when_false, immutable, diagnostics);
        }
        Expression::Binary { left, right, .. } => {
            analyze_expression(left, immutable, diagnostics);
            analyze_expression(right, immutable, diagnostics);
        }
        Expression::Call { callee, arguments, .. } => {
            analyze_expression(callee, immutable, diagnostics);
            for argument in arguments { analyze_expression(argument, immutable, diagnostics); }
        }
        Expression::ObjectCreation { arguments, .. } => {
            for argument in arguments { analyze_expression(argument, immutable, diagnostics); }
        }
        Expression::Member { object, .. } => analyze_expression(object, immutable, diagnostics),
        Expression::Index { object, index, .. } => {
            analyze_expression(object, immutable, diagnostics);
            analyze_expression(index, immutable, diagnostics);
        }
        Expression::Null(span) => diagnostics.push(Diagnostic::error_with_code(
            "NX1102", "Das null-Literal ist in sicherem NORTEX-Code nicht zulässig. Verwende Option<T>, Some(...) oder None.", *span)),
        Expression::Identifier(_, _) | Expression::Integer(_, _) | Expression::Float(_, _) |
        Expression::String(_, _) | Expression::Character(_, _) | Expression::Boolean(_, _) => {}
    }
}

pub(super) fn root_identifier(expression: &Expression) -> Option<&str> {
    match expression {
        Expression::Identifier(name, _) => Some(name),
        Expression::Member { object, .. } | Expression::Index { object, .. } => {
            root_identifier(object)
        }
        Expression::Group(inner, _) => root_identifier(inner),
        _ => None,
    }
}

pub(super) fn contains_raw_pointer(ty: &TypeReference) -> bool {
    matches!(
        ty.reference_kind,
        ReferenceKind::RawConstPointer | ReferenceKind::RawMutablePointer
    ) || ty.generic_arguments.iter().any(contains_raw_pointer)
}

pub(super) fn is_constant_expression(expression: &Expression) -> bool {
    match expression {
        Expression::Integer(..)
        | Expression::Float(..)
        | Expression::String(..)
        | Expression::Character(..)
        | Expression::Boolean(..) => true,
        Expression::Unary {
            operator, operand, ..
        } => matches!(operator.as_str(), "+" | "-" | "!" | "~") && is_constant_expression(operand),
        Expression::Binary {
            left,
            operator,
            right,
            ..
        } => {
            matches!(
                operator.as_str(),
                "+" | "-"
                    | "*"
                    | "/"
                    | "%"
                    | "=="
                    | "!="
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "&"
                    | "|"
                    | "^"
                    | "<<"
                    | ">>"
            ) && is_constant_expression(left)
                && is_constant_expression(right)
        }
        Expression::Group(inner, _) => is_constant_expression(inner),
        Expression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            is_constant_expression(condition)
                && is_constant_expression(when_true)
                && is_constant_expression(when_false)
        }
        // Constant-name references are deliberately not accepted yet: the
        // compiler needs a dependency graph and cycle detection first.
        _ => false,
    }
}

pub(super) fn validate_type(
    ty: &TypeReference,
    known: &HashMap<String, KnownType>,
    generics: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !generics.contains(&ty.name) {
        let known_type = known
            .get(&ty.name)
            .or_else(|| ty.name.rsplit('.').next().and_then(|name| known.get(name)));
        match known_type {
            Some(info) => {
                let supplied = ty.generic_arguments.len();
                if supplied < info.min_generic_arity || supplied > info.max_generic_arity {
                    let expected = if info.min_generic_arity == info.max_generic_arity {
                        info.min_generic_arity.to_string()
                    } else {
                        format!("{} bis {}", info.min_generic_arity, info.max_generic_arity)
                    };
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1101",
                        format!(
                            "Typ '{}' erwartet {} generische Argumente, erhalten: {}.",
                            ty.name, expected, supplied
                        ),
                        ty.span,
                    ));
                }
            }
            None => {
                let suggestion = canonical_type_suggestion(&ty.name);
                let message = suggestion.map_or_else(
                    || format!("Unbekannter Typ '{}'. Typen müssen in NORTEX definiert oder importiert sein.", ty.name),
                    |canonical| format!("Unbekannter Typ '{}'. Meintest du '{}'?", ty.name, canonical),
                );
                diagnostics.push(Diagnostic::error_with_code("NX1100", message, ty.span));
            }
        }
    }
    for argument in &ty.generic_arguments {
        validate_type(argument, known, generics, diagnostics);
    }
}

pub(super) fn canonical_type_suggestion(name: &str) -> Option<&'static str> {
    match name {
        "SByte" | "Int8" | "int8" => Some("sbyte"),
        "Int16" => Some("short"),
        "Int32" => Some("int"),
        "Int64" => Some("long"),
        "Int128" => Some("i128"),
        "Byte" | "UInt8" | "uint8" => Some("byte"),
        "UInt16" => Some("ushort"),
        "UInt32" => Some("uint"),
        "UInt64" => Some("ulong"),
        "UInt128" => Some("u128"),
        "Single" | "Float32" => Some("float"),
        "Double" | "Float64" => Some("double"),
        "Boolean" => Some("bool"),
        "String" => Some("string"),
        "Char" => Some("char"),
        "Unit" => Some("void"),
        _ => None,
    }
}

pub(super) fn validate_type_name(
    name: &str,
    kind: TypeDeclarationKind,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let first = name.chars().next();
    if !first.is_some_and(|c| c.is_ascii_uppercase())
        || name.contains('_')
        || has_long_acronym(name)
    {
        diagnostics.push(Diagnostic::warning(
            format!(
                "{} '{}' sollte in PascalCase geschrieben werden (z. B. XmlParser, IOStream).",
                type_kind_name(kind),
                name
            ),
            span,
        ));
    }
}

pub(super) fn validate_value_name(name: &str, span: SourceSpan, diagnostics: &mut Vec<Diagnostic>) {
    if name == "main" {
        return;
    }
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        || name.contains('_')
        || has_long_acronym(name)
    {
        diagnostics.push(Diagnostic::warning(
            format!("Member oder Variable '{name}' sollte in camelCase geschrieben werden (z. B. parseHttpHeader)."),
            span,
        ));
    }
}

pub(super) fn has_long_acronym(name: &str) -> bool {
    let mut uppercase_run = 0usize;
    for character in name.chars() {
        if character.is_ascii_uppercase() {
            uppercase_run += 1;
        } else {
            if character.is_ascii_lowercase() && uppercase_run.saturating_sub(1) >= 3 {
                return true;
            }
            uppercase_run = 0;
        }
    }
    uppercase_run >= 3
}

pub(super) fn validate_constant_name(
    name: &str,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        diagnostics.push(Diagnostic::warning(
            format!("Konstante '{name}' sollte in UPPER_SNAKE_CASE geschrieben werden."),
            span,
        ));
    }
}

pub(super) fn validate_identifier(
    name: &str,
    role: &str,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        || name.contains('_')
        || has_long_acronym(name)
    {
        diagnostics.push(Diagnostic::warning(
            format!(
                "{role} '{name}' sollte in PascalCase geschrieben werden (z. B. TValue, IOStream)."
            ),
            span,
        ));
    }
}

pub(super) fn declaration_span(declaration: &Declaration) -> SourceSpan {
    match declaration {
        Declaration::Class(class) => class.span,
        Declaration::Function(function) => function.span,
        Declaration::GlobalVariable(global) => global.span,
    }
}

pub(super) fn type_kind_name(kind: TypeDeclarationKind) -> &'static str {
    match kind {
        TypeDeclarationKind::Class => "Klasse",
        TypeDeclarationKind::Struct => "Struktur",
        TypeDeclarationKind::Record => "Record",
        TypeDeclarationKind::Interface => "Interface",
        TypeDeclarationKind::Trait => "Trait",
        TypeDeclarationKind::Enum => "Enum",
    }
}

pub(super) fn function_signature(function: &FunctionDeclaration) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| type_reference_signature(&parameter.type_ref))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}#{}({})",
        function.name,
        function.generic_parameters.len(),
        parameters
    )
}

pub(super) fn function_contract_signature(function: &FunctionDeclaration) -> String {
    format!(
        "{} -> {}",
        function_signature(function),
        type_reference_signature(&function.return_type)
    )
}

pub(super) fn type_reference_signature(ty: &TypeReference) -> String {
    let generic_arguments = if ty.generic_arguments.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            ty.generic_arguments
                .iter()
                .map(type_reference_signature)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let arrays = if let Some(length) = ty.fixed_array_length {
        format!("[;{length}]")
    } else {
        "[]".repeat(ty.array_depth)
    };
    format!(
        "{:?}:{}{}{}",
        ty.reference_kind, ty.name, generic_arguments, arrays
    )
}

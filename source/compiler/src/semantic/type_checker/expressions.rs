use super::*;

impl Checker<'_> {
    pub(super) fn expect_expression(
        &mut self,
        expression: &Expression,
        expected: &str,
        scopes: &mut Vec<Scope>,
        owner: Option<&str>,
        is_async: bool,
        context: &str,
    ) {
        if let Some(actual) = self.infer(expression, scopes, owner, is_async) {
            self.expect_type(expected, &actual, expression.span(), context);
        }
    }

    /// Checks an initializer, return value or field value. Unsuffixed integer
    /// literals adapt to any integer or floating-point target type if the
    /// value fits; otherwise a range diagnostic is reported.
    pub(super) fn expect_value(
        &mut self,
        expected: &str,
        actual: &str,
        value: &Expression,
        context: &str,
    ) {
        if self.literal_accepted(expected, value) {
            return;
        }
        self.expect_type(expected, actual, value.span(), context);
    }

    /// Returns true if `value` is an unsuffixed integer literal that is
    /// accepted for `expected`. Emits NX1102 if the literal is out of range.
    pub(super) fn literal_accepted(&mut self, expected: &str, value: &Expression) -> bool {
        let Some((literal, negative)) = unsuffixed_integer_literal(value) else {
            return false;
        };
        if matches!(expected, "float" | "double" | "decimal") {
            return true;
        }
        let Some((min, max)) = integer_range(expected) else {
            return false;
        };
        match parse_integer_literal(literal) {
            Some(magnitude) => {
                let fits = if negative {
                    magnitude <= min.unsigned_abs() && (min < 0 || magnitude == 0)
                } else {
                    magnitude <= max
                };
                if !fits {
                    let shown = if negative {
                        format!("-{literal}")
                    } else {
                        literal.to_string()
                    };
                    self.emit("NX1102", format!("Das Literal {shown} liegt außerhalb des Wertebereichs von '{expected}'."), value.span());
                }
                true
            }
            None => {
                self.emit(
                    "NX1102",
                    format!("Das Literal {literal} ist zu groß für jeden ganzzahligen Typ."),
                    value.span(),
                );
                true
            }
        }
    }

    /// Structural assignability: identical types, `dynamic`, unsubstituted
    /// generic parameters, generic arguments pairwise, and nominal subtyping
    /// over declared base classes and interfaces.
    pub(super) fn is_assignable(&self, expected: &str, actual: &str) -> bool {
        if expected == actual
            || expected == "dynamic"
            || actual == "dynamic"
            || expected == "<unknown>"
            || actual == "<unknown>"
            || self.type_parameters.contains(expected)
            || self.type_parameters.contains(actual)
        {
            return true;
        }
        if actual == "{integer}" {
            return integer_range(expected).is_some()
                || matches!(expected, "float" | "double" | "decimal");
        }
        let (expected_name, expected_args) = split_generic(expected);
        let (actual_name, actual_args) = split_generic(actual);
        if expected_name == actual_name
            && expected_args.len() == actual_args.len()
            && !expected_args.is_empty()
        {
            return expected_args.iter().zip(&actual_args).all(|(left, right)| {
                left == right
                    || *left == "dynamic"
                    || *right == "dynamic"
                    || self.type_parameters.contains(*left)
                    || self.type_parameters.contains(*right)
                    || (!split_generic(left).1.is_empty() && self.is_assignable(left, right))
            });
        }
        if expected_args.is_empty() && actual_args.is_empty() {
            return self.is_subtype(actual_name, expected_name, 0);
        }
        false
    }

    pub(super) fn is_subtype(&self, actual: &str, expected: &str, depth: usize) -> bool {
        if depth > self.classes.len() {
            return false;
        }
        let Some(class) = self.classes.get(actual) else {
            return false;
        };
        class
            .supertypes
            .iter()
            .any(|base| base == expected || self.is_subtype(base, expected, depth + 1))
    }

    pub(super) fn expect_type(
        &mut self,
        expected: &str,
        actual: &str,
        span: SourceSpan,
        context: &str,
    ) {
        if self.is_assignable(expected, actual)
            || !self.known_types.contains(nominal_type_name(expected))
            || !self.known_types.contains(nominal_type_name(actual))
        {
            return;
        }
        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1102" && diagnostic.span == span)
        {
            return;
        }
        self.emit(
            "NX1102",
            format!("Typfehler in {context}: erwartet '{expected}', erhalten '{actual}'."),
            span,
        );
    }

    pub(super) fn validate_type_reference(&mut self, ty: &TypeReference) {
        if !self.known_types.contains(&ty.name) {
            self.emit(
                "NX1100",
                format!(
                    "Unbekannter Typ '{}'. Meintest du einen der kanonischen NORTEX-Typen?",
                    ty.name
                ),
                ty.span,
            );
        }
        for argument in &ty.generic_arguments {
            self.validate_type_reference(argument);
        }
    }

    pub(super) fn infer(
        &mut self,
        expression: &Expression,
        scopes: &mut Vec<Scope>,
        owner: Option<&str>,
        is_async: bool,
    ) -> Option<String> {
        match expression {
            Expression::Integer(value, _) => Some(integer_literal_type(value)),
            Expression::Float(value, _) => {
                let lower = value.to_ascii_lowercase();
                if lower.ends_with('f') {
                    Some("float".to_string())
                } else if lower.ends_with("dec") || lower.ends_with('m') {
                    Some("decimal".to_string())
                } else {
                    Some("double".to_string())
                }
            }
            Expression::String(_, _) => Some("string".to_string()),
            Expression::Character(_, _) => Some("char".to_string()),
            Expression::Boolean(_, _) => Some("bool".to_string()),
            Expression::Null(_) => None,
            Expression::Identifier(name, span) => {
                if let Some(binding) = scopes.iter().rev().find_map(|scope| scope.get(name)) {
                    if binding.id.is_some_and(|id| self.moved_values.contains(&id)) {
                        self.emit(
                            "NX1202",
                            format!("Der Wert '{name}' wurde bereits verschoben und kann nicht erneut verwendet werden."),
                            *span,
                        );
                    }
                    return Some(binding.type_name.clone());
                }
                if name == "this" {
                    return owner.map(str::to_string);
                }
                if [
                    "Console",
                    "Text",
                    "Convert",
                    "File",
                    "Directory",
                    "IntList",
                    "StringList",
                    "Math",
                    "Time",
                    "Debug",
                    "Environment",
                    "Process",
                    "System",
                ]
                .contains(&name.as_str())
                {
                    return Some(format!("@static:{name}"));
                }
                if self.classes.contains_key(name) {
                    return Some(format!("@static:{name}"));
                }
                if self.functions.contains_key(name) {
                    return Some("@function".to_string());
                }
                if let Some(type_name) = self.global_variables.get(name) {
                    return Some(type_name.clone());
                }
                None
            }
            Expression::Group(inner, _) => self.infer(inner, scopes, owner, is_async),
            Expression::Unary {
                operator,
                operand,
                span,
            } => {
                let actual = self.infer(operand, scopes, owner, is_async)?;
                match operator.as_str() {
                    "await" => {
                        if !is_async {
                            self.emit(
                                "NX1107",
                                "await ist nur innerhalb einer async-Funktion zulässig.",
                                *span,
                            );
                            return None;
                        }
                        task_result_type(&actual).map(str::to_string).or_else(|| {
                            self.emit(
                                "NX1107",
                                format!("await erwartet Task<T>, erhalten wurde '{actual}'."),
                                *span,
                            );
                            None
                        })
                    }
                    "!" => {
                        self.expect_type("bool", &actual, operand.span(), "unärer Operator '!'");
                        Some("bool".to_string())
                    }
                    "+" | "-" | "++" | "--" | "post++" | "post--" => {
                        if !is_numeric_type(&actual) {
                            self.emit("NX1102", format!("Operator '{operator}' benötigt einen numerischen Wert, erhalten wurde '{actual}'."), *span);
                        }
                        Some(actual)
                    }
                    "&" => Some(format!("&{actual}")),
                    "&mut" => Some(format!("&mut {actual}")),
                    "*" => actual
                        .strip_prefix("&mut ")
                        .or_else(|| actual.strip_prefix('&'))
                        .or_else(|| actual.strip_prefix("*const "))
                        .or_else(|| actual.strip_prefix("*mut "))
                        .map(str::to_string),
                    _ => Some(actual),
                }
            }
            Expression::Binary {
                left,
                operator,
                right,
                span,
            } => {
                let mut left_type = self.infer(left, scopes, owner, is_async)?;
                let mut right_type = self.infer(right, scopes, owner, is_async)?;
                if left_type != right_type
                    && is_numeric_type(&left_type)
                    && is_numeric_type(&right_type)
                {
                    if self.literal_accepted(&left_type, right) {
                        right_type = left_type.clone();
                    } else if self.literal_accepted(&right_type, left) {
                        left_type = right_type.clone();
                    }
                }
                match operator.as_str() {
                    "&&" | "||" => {
                        self.expect_type("bool", &left_type, left.span(), "logischer Operator");
                        self.expect_type("bool", &right_type, right.span(), "logischer Operator");
                        Some("bool".to_string())
                    }
                    "==" | "!=" => {
                        self.expect_type(&left_type, &right_type, *span, "Vergleich");
                        Some("bool".to_string())
                    }
                    "<" | ">" | "<=" | ">=" => {
                        if left_type != right_type
                            || !(is_numeric_type(&left_type)
                                || matches!(left_type.as_str(), "string" | "char"))
                        {
                            self.emit("NX1102", format!("Die Operanden von '{operator}' müssen kompatible Zahlen, char oder string sein."), *span);
                        }
                        Some("bool".to_string())
                    }
                    "+" if left_type == "string" && right_type == "string" => {
                        Some("string".to_string())
                    }
                    "+" if left_type == "string" && is_string_concatenable(&right_type)
                        || right_type == "string" && is_string_concatenable(&left_type) =>
                    {
                        Some("string".to_string())
                    }
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>" => {
                        if left_type != right_type || !is_numeric_type(&left_type) {
                            self.emit("NX1102", format!("Die Operanden von '{operator}' müssen denselben numerischen Typ besitzen; erhalten wurden '{left_type}' und '{right_type}'."), *span);
                            None
                        } else {
                            Some(left_type)
                        }
                    }
                    _ => Some(left_type),
                }
            }
            Expression::Conditional {
                condition,
                when_true,
                when_false,
                span,
            } => {
                self.expect_expression(
                    condition,
                    "bool",
                    scopes,
                    owner,
                    is_async,
                    "ternäre Bedingung",
                );
                let true_type = self.infer(when_true, scopes, owner, is_async)?;
                let false_type = self.infer(when_false, scopes, owner, is_async)?;
                self.expect_type(&true_type, &false_type, *span, "ternäre Zweige");
                Some(true_type)
            }
            Expression::Assignment {
                target,
                value,
                span,
                ..
            } => {
                let target_name = match target.as_ref() {
                    Expression::Identifier(name, _) => Some(name.as_str()),
                    _ => None,
                };
                let target_binding = target_name.and_then(|name| {
                    scopes
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(name))
                        .cloned()
                });
                if let Expression::Unary {
                    operator, operand, ..
                } = target.as_ref()
                    && operator == "*"
                    && let Expression::Identifier(name, _) = operand.as_ref()
                    && let Some(binding) = scopes.iter().rev().find_map(|scope| scope.get(name))
                    && !binding.type_name.starts_with("&mut ")
                {
                    self.emit(
                        "NX1200",
                        format!("Zuweisung durch '{name}' erfordert eine veränderliche Referenz."),
                        *span,
                    );
                }
                let target_type = match target_binding.as_ref() {
                    Some(binding) => binding.type_name.clone(),
                    None => self.infer(target, scopes, owner, is_async)?,
                };
                let value_type = self.infer(value, scopes, owner, is_async)?;
                if !self.literal_accepted(&target_type, value) {
                    self.expect_type(&target_type, &value_type, *span, "Zuweisung");
                }
                self.move_value_expression(value, &target_type, scopes);
                if let Some(id) = target_binding.and_then(|binding| binding.id) {
                    self.moved_values.remove(&id);
                }
                Some(target_type)
            }
            Expression::Call {
                callee,
                generic_arguments,
                arguments,
                span,
            } => {
                for generic_argument in generic_arguments {
                    self.validate_type_reference(generic_argument);
                }
                let argument_types = self.argument_types(arguments, scopes, owner, is_async);
                let call_arguments = CallArguments {
                    generic_arguments,
                    expressions: arguments,
                    types: &argument_types,
                    span: *span,
                };
                self.infer_call(callee, &call_arguments, scopes, owner, is_async)
            }
            Expression::ObjectCreation {
                type_ref,
                arguments,
                span,
            } => {
                let argument_types = self.argument_types(arguments, scopes, owner, is_async);
                let name = type_name(type_ref);
                let generic_arguments = type_ref
                    .generic_arguments
                    .iter()
                    .map(type_name)
                    .collect::<Vec<_>>();
                self.check_constructor(&type_ref.name, &generic_arguments, &argument_types, *span);
                Some(name)
            }
            Expression::Member { object, name, span } => {
                let receiver = self.infer(object, scopes, owner, is_async)?;
                self.member_type(&receiver, name, *span, owner)
            }
            Expression::Index {
                object,
                index,
                span,
            } => {
                let object_type = self.infer(object, scopes, owner, is_async)?;
                let index_type = self.infer(index, scopes, owner, is_async)?;
                if !matches!(index_type.as_str(), "int" | "uint" | "usize" | "long") {
                    self.emit("NX1102", format!("Ein Index benötigt einen ganzzahligen Typ, erhalten wurde '{index_type}'."), index.span());
                }
                if object_type == "string" {
                    Some("byte".to_string())
                } else if let Some(element) = object_type.strip_suffix("[]") {
                    Some(element.to_string())
                } else if object_type.starts_with("[;") {
                    None
                } else {
                    self.emit(
                        "NX1106",
                        format!("Typ '{object_type}' unterstützt keinen Indexzugriff."),
                        *span,
                    );
                    None
                }
            }
        }
    }
}

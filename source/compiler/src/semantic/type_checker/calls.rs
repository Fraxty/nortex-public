use super::*;

impl Checker<'_> {
    pub(super) fn infer_call(
        &mut self,
        callee: &Expression,
        call: &CallArguments<'_>,
        scopes: &mut Vec<Scope>,
        owner: Option<&str>,
        is_async: bool,
    ) -> Option<String> {
        match callee {
            Expression::Identifier(name, _) => {
                if let Some(overloads) = self.functions.get(name).cloned() {
                    let overloads = self.specialize_generic_overloads(
                        &overloads,
                        call.generic_arguments,
                        call.types,
                        call.span,
                    );
                    return self.resolve_call_overload(
                        &overloads,
                        call.types,
                        call.expressions,
                        scopes,
                        call.span,
                        name,
                    );
                }
                if let Some(owner_name) = owner
                    && let Some(methods) = self.find_methods(owner_name, name, 0)
                {
                    let methods = accessible_methods(&methods, owner);
                    if !methods.is_empty() {
                        let methods = self.specialize_generic_overloads(
                            &methods,
                            call.generic_arguments,
                            call.types,
                            call.span,
                        );
                        return self.resolve_call_overload(
                            &methods,
                            call.types,
                            call.expressions,
                            scopes,
                            call.span,
                            name,
                        );
                    }
                }
                match name.as_str() {
                    "print" | "println" | "assert" | "cleanup" | "writeText" => {
                        Some("void".to_string())
                    }
                    "read" => Some("string".to_string()),
                    "readText" => Some("Task<string>".to_string()),
                    "Some" => call
                        .types
                        .first()
                        .and_then(Clone::clone)
                        .map(|ty| format!("Option<{}>", literal_default(&ty))),
                    "Ok" => call
                        .types
                        .first()
                        .and_then(Clone::clone)
                        .map(|ty| format!("Result<{}, dynamic>", literal_default(&ty))),
                    "Err" => call
                        .types
                        .first()
                        .and_then(Clone::clone)
                        .map(|ty| format!("Result<dynamic, {}>", literal_default(&ty))),
                    _ if self.classes.contains_key(name) || is_builtin_nominal_type(name) => {
                        // `Player()` is the plan's construction syntax.
                        let generic_arguments = call
                            .generic_arguments
                            .iter()
                            .map(type_name)
                            .collect::<Vec<_>>();
                        if let Some(class) = self.classes.get(name)
                            && !generic_arguments.is_empty()
                            && class.generics.len() != generic_arguments.len()
                        {
                            self.emit(
                                "NX1107",
                                format!(
                                    "Typ '{name}' erwartet {} generische Argumente, erhalten wurden {}.",
                                    class.generics.len(),
                                    generic_arguments.len()
                                ),
                                call.span,
                            );
                            return None;
                        }
                        self.check_constructor(name, &generic_arguments, call.types, call.span);
                        Some(if generic_arguments.is_empty() {
                            name.clone()
                        } else {
                            format!("{name}<{}>", generic_arguments.join(", "))
                        })
                    }
                    _ => {
                        let callee_type = self.infer(callee, scopes, owner, is_async);
                        if callee_type.is_some() {
                            self.emit(
                                "NX1107",
                                format!("Der Wert '{name}' ist nicht aufrufbar."),
                                call.span,
                            );
                        }
                        None
                    }
                }
            }
            Expression::Member { object, name, .. } => {
                let receiver = self.infer(object, scopes, owner, is_async)?;
                if let Some(static_type) = receiver.strip_prefix("@static:") {
                    return self.static_method_type(static_type, name, call, scopes);
                }
                if receiver == "string" {
                    return match name.as_str() {
                        "chars" => Some("Iterator<char>".to_string()),
                        "bytes" => Some("Iterator<byte>".to_string()),
                        "isEmpty" => Some("bool".to_string()),
                        "length" => Some("usize".to_string()),
                        "contains" | "startsWith" | "endsWith" => Some("bool".to_string()),
                        _ => {
                            self.emit(
                                "NX1106",
                                format!("Unbekanntes string-Member '{name}'."),
                                call.span,
                            );
                            None
                        }
                    };
                }
                let base = nominal_type_name(&receiver);
                if self.classes.contains_key(base) {
                    if let Some(methods) = self.find_methods(base, name, 0) {
                        let methods = accessible_methods(&methods, owner);
                        if methods.is_empty() {
                            self.emit(
                                "NX1105",
                                format!("Methode '{base}.{name}' ist außerhalb ihres deklarierenden Typs nicht zugänglich."),
                                call.span,
                            );
                            return None;
                        }
                        let methods = self.substitute_for_receiver(&receiver, methods);
                        let methods = self.specialize_generic_overloads(
                            &methods,
                            call.generic_arguments,
                            call.types,
                            call.span,
                        );
                        return self.resolve_call_overload(
                            &methods,
                            call.types,
                            call.expressions,
                            scopes,
                            call.span,
                            name,
                        );
                    }
                    self.emit(
                        "NX1106",
                        format!("Typ '{base}' besitzt keine Methode '{name}'."),
                        call.span,
                    );
                    return None;
                }
                let element = split_generic(&receiver)
                    .1
                    .first()
                    .map(|value| value.to_string());
                match (base, name.as_str()) {
                    ("List" | "Vec", "add" | "push") => {
                        if let (Some(element), Some(Some(actual))) =
                            (element.as_deref(), call.types.first())
                            && (call.types.len() != 1 || !self.is_assignable(element, actual))
                        {
                            self.emit("NX1107", format!("'{base}<{element}>.{name}' erwartet genau ein Argument vom Typ '{element}', erhalten wurde '{}'.", literal_default(actual)), call.span);
                        }
                        Some("void".to_string())
                    }
                    ("List" | "Vec", "clear" | "reserve" | "shrinkToFit") => {
                        Some("void".to_string())
                    }
                    ("List" | "Vec", "get" | "getMutable" | "remove" | "pop") => {
                        Some("dynamic".to_string())
                    }
                    ("Dictionary", "containsKey") => Some("bool".to_string()),
                    _ if is_numeric_type(base) || matches!(base, "bool" | "char" | "byte") => {
                        self.emit(
                            "NX1106",
                            format!("Typ '{base}' besitzt keine Methode '{name}'."),
                            call.span,
                        );
                        None
                    }
                    _ => None,
                }
            }
            _ => {
                self.infer(callee, scopes, owner, is_async);
                None
            }
        }
    }

    pub(super) fn member_type(
        &mut self,
        receiver: &str,
        name: &str,
        span: SourceSpan,
        accessing_type: Option<&str>,
    ) -> Option<String> {
        if receiver.starts_with("@static:") {
            return Some("@method".to_string());
        }
        if receiver == "string" && name == "length" {
            return Some("usize".to_string());
        }
        let base = nominal_type_name(receiver);
        if let Some((field, declaring_type, is_private)) = self.find_field(base, name, 0) {
            if is_private && accessing_type != Some(declaring_type.as_str()) {
                self.emit(
                    "NX1105",
                    format!("Feld '{base}.{name}' ist außerhalb seines deklarierenden Typs nicht zugänglich."),
                    span,
                );
                return None;
            }
            let map = self.generic_map(receiver);
            return Some(substitute(&field, &map));
        }
        if self.classes.contains_key(base)
            || is_builtin_nominal_type(base)
            || is_numeric_type(base)
            || matches!(base, "bool" | "char" | "byte" | "string")
        {
            self.emit(
                "NX1106",
                format!("Typ '{base}' besitzt kein Feld '{name}'."),
                span,
            );
        }
        None
    }

    pub(super) fn find_methods(
        &self,
        class_name: &str,
        name: &str,
        depth: usize,
    ) -> Option<Vec<FunctionType>> {
        if depth > self.classes.len() {
            return None;
        }
        let class = self.classes.get(class_name)?;
        class.methods.get(name).cloned().or_else(|| {
            class
                .base
                .as_deref()
                .and_then(|base| self.find_methods(base, name, depth + 1))
        })
    }

    pub(super) fn find_field(
        &self,
        class_name: &str,
        name: &str,
        depth: usize,
    ) -> Option<(String, String, bool)> {
        if depth > self.classes.len() {
            return None;
        }
        let class = self.classes.get(class_name)?;
        if let Some(field) = class.fields.get(name) {
            return Some((
                field.clone(),
                class_name.to_string(),
                class.private_fields.contains(name),
            ));
        }
        class
            .base
            .as_deref()
            .and_then(|base| self.find_field(base, name, depth + 1))
    }

    pub(super) fn static_method_type(
        &mut self,
        owner: &str,
        name: &str,
        call: &CallArguments<'_>,
        scopes: &[Scope],
    ) -> Option<String> {
        if owner == "Console" {
            let overloads = match name {
                "write" | "writeLine" => ["string", "char", "bool", "int", "long", "double"]
                    .into_iter()
                    .map(|parameter| FunctionType {
                        parameters: vec![parameter.to_string()],
                        result: "void".to_string(),
                        ..FunctionType::default()
                    })
                    .collect::<Vec<_>>(),
                "readLine" => vec![FunctionType {
                    parameters: Vec::new(),
                    result: "string".to_string(),
                    ..FunctionType::default()
                }],
                _ => {
                    self.emit(
                        "NX1106",
                        format!("Unbekanntes Console-Member '{name}'."),
                        call.span,
                    );
                    return None;
                }
            };
            let overloads = self.specialize_generic_overloads(
                &overloads,
                call.generic_arguments,
                call.types,
                call.span,
            );
            return self.resolve_call_overload(
                &overloads,
                call.types,
                call.expressions,
                scopes,
                call.span,
                &format!("Console.{name}"),
            );
        }
        if let Some(builtin) = crate::builtins::find(&format!("{owner}.{name}")) {
            let matches = builtin.params.len() == call.types.len()
                && builtin
                    .params
                    .iter()
                    .zip(call.types)
                    .all(|(expected, actual)| {
                        actual.as_deref().is_none_or(|actual| {
                            self.is_assignable(expected, &literal_default(actual))
                        })
                    });
            if !matches {
                self.emit(
                    "NX1107",
                    format!("'{owner}.{name}' erwartet ({}).", builtin.params.join(", ")),
                    call.span,
                );
                return None;
            }
            return Some(builtin.returns.to_owned());
        }
        if let Some(class) = self.classes.get(owner).cloned() {
            if let Some(methods) = class.methods.get(name) {
                let static_methods = methods
                    .iter()
                    .filter(|method| method.is_static)
                    .cloned()
                    .collect::<Vec<_>>();
                if static_methods.is_empty() {
                    self.emit(
                        "NX1107",
                        format!(
                            "Instanzmethode '{owner}.{name}' kann nicht statisch aufgerufen werden."
                        ),
                        call.span,
                    );
                    return None;
                }
                let static_methods = accessible_methods(&static_methods, None);
                let static_methods = self.specialize_generic_overloads(
                    &static_methods,
                    call.generic_arguments,
                    call.types,
                    call.span,
                );
                return self.resolve_call_overload(
                    &static_methods,
                    call.types,
                    call.expressions,
                    scopes,
                    call.span,
                    name,
                );
            }
            self.emit(
                "NX1106",
                format!("Typ '{owner}' besitzt keine statische Methode '{name}'."),
                call.span,
            );
        }
        None
    }

    pub(super) fn specialize_generic_overloads(
        &mut self,
        overloads: &[FunctionType],
        generic_arguments: &[TypeReference],
        arguments: &[Option<String>],
        span: SourceSpan,
    ) -> Vec<FunctionType> {
        let explicit_arguments = generic_arguments.iter().map(type_name).collect::<Vec<_>>();
        let mut specialized = Vec::new();
        let mut arity_error = false;
        for overload in overloads {
            if overload.generic_parameters.is_empty() && explicit_arguments.is_empty() {
                specialized.push(overload.clone());
                continue;
            }
            if !explicit_arguments.is_empty()
                && explicit_arguments.len() != overload.generic_parameters.len()
            {
                arity_error = true;
                continue;
            }

            let mut substitutions = HashMap::new();
            if !explicit_arguments.is_empty() {
                substitutions.extend(
                    overload
                        .generic_parameters
                        .iter()
                        .cloned()
                        .zip(explicit_arguments.iter().cloned()),
                );
            } else {
                for (parameter, actual) in overload.parameters.iter().zip(arguments) {
                    if overload
                        .generic_parameters
                        .iter()
                        .any(|generic| generic == parameter)
                        && let Some(actual) = actual
                    {
                        substitutions
                            .entry(parameter.clone())
                            .or_insert_with(|| literal_default(actual));
                    }
                }
            }
            let mut candidate = substitute_function(overload, &substitutions);
            candidate
                .generic_parameters
                .retain(|generic| !substitutions.contains_key(generic));
            specialized.push(candidate);
        }
        if specialized.is_empty() && arity_error {
            let expected = overloads
                .first()
                .map_or(0, |overload| overload.generic_parameters.len());
            self.emit(
                "NX1107",
                format!(
                    "Generischer Aufruf erwartet {expected} Typargumente, erhalten wurden {}.",
                    explicit_arguments.len()
                ),
                span,
            );
        }
        specialized
    }

    pub(super) fn resolve_call_overload(
        &mut self,
        overloads: &[FunctionType],
        arguments: &[Option<String>],
        argument_expressions: &[Expression],
        scopes: &[Scope],
        span: SourceSpan,
        display_name: &str,
    ) -> Option<String> {
        let viable = overloads
            .iter()
            .filter(|candidate| {
                candidate.parameters.len() == arguments.len()
                    && candidate
                        .parameters
                        .iter()
                        .zip(arguments)
                        .all(|(expected, actual)| {
                            actual.as_deref().is_none_or(|actual| {
                                self.is_assignable(expected, &literal_default(actual))
                            })
                        })
            })
            .collect::<Vec<_>>();
        let best_score = viable
            .iter()
            .filter_map(|candidate| overload_score(candidate, arguments))
            .min();
        let selected = best_score.and_then(|best_score| {
            let mut best = viable
                .into_iter()
                .filter(|candidate| overload_score(candidate, arguments) == Some(best_score));
            let selected = best.next();
            if best.next().is_none() {
                selected
            } else {
                None
            }
        });
        let result = self.resolve_overload(overloads, arguments, span, display_name);
        if result.is_some()
            && let Some(selected) = selected
        {
            self.validate_call_borrows(&selected.parameters, argument_expressions, span);
            // Konsolenausgabe leiht ihre Argumente nur aus.
            if !display_name.starts_with("Console.") {
                self.mark_consumed_arguments(&selected.parameters, argument_expressions, scopes);
            }
        }
        result
    }

    pub(super) fn validate_call_borrows(
        &mut self,
        parameters: &[String],
        arguments: &[Expression],
        span: SourceSpan,
    ) {
        let mut borrows = Vec::<(BorrowPlace, bool)>::new();
        let mut conflict = None;
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !parameter.starts_with('&') {
                continue;
            }
            let Some((place, is_mutable)) = borrow_argument(argument, parameter) else {
                continue;
            };
            if borrows.iter().any(|(previous, previous_mutable)| {
                (is_mutable || *previous_mutable) && borrow_places_overlap(previous, &place)
            }) {
                conflict = Some(place);
                break;
            }
            borrows.push((place, is_mutable));
        }
        if let Some(place) = conflict {
            let shown = place.root.as_deref().unwrap_or("<unbekannter Speicherort>");
            self.emit(
                "NX1203",
                format!("Speicherort '{shown}' kann in einem Aufruf nicht gleichzeitig geteilt und exklusiv ausgeliehen werden."),
                span,
            );
        }
    }

    pub(super) fn mark_consumed_arguments(
        &mut self,
        parameters: &[String],
        arguments: &[Expression],
        scopes: &[Scope],
    ) {
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if parameter.starts_with('&') || parameter.starts_with('*') {
                continue;
            }
            let Some(name) = move_source_identifier(argument) else {
                continue;
            };
            let Some(binding) = scopes.iter().rev().find_map(|scope| scope.get(name)) else {
                continue;
            };
            if !self.classes.contains_key(&binding.type_name)
                && requires_move(&binding.type_name)
                && let Some(id) = binding.id
            {
                self.moved_values.insert(id);
            }
        }
    }

    pub(super) fn move_value_expression(
        &mut self,
        expression: &Expression,
        ty: &str,
        scopes: &[Scope],
    ) {
        if self.classes.contains_key(ty) || !requires_move(ty) {
            return;
        }
        if let Some(name) = move_source_identifier(expression)
            && let Some(binding) = scopes.iter().rev().find_map(|scope| scope.get(name))
            && let Some(id) = binding.id
        {
            self.moved_values.insert(id);
        }
    }

    pub(super) fn resolve_overload(
        &mut self,
        overloads: &[FunctionType],
        arguments: &[Option<String>],
        span: SourceSpan,
        display_name: &str,
    ) -> Option<String> {
        let arity_matches = overloads
            .iter()
            .filter(|candidate| candidate.parameters.len() == arguments.len())
            .collect::<Vec<_>>();
        // Exact matches first (an integer literal counts as `int`), then
        // assignable matches (subtypes, generic wildcards, literal widening).
        let exact = arity_matches
            .iter()
            .copied()
            .filter(|candidate| {
                candidate
                    .parameters
                    .iter()
                    .zip(arguments)
                    .all(|(expected, actual)| {
                        actual
                            .as_deref()
                            .is_none_or(|actual| *expected == literal_default(actual))
                    })
            })
            .collect::<Vec<_>>();
        let matching = if exact.len() == 1 {
            exact
        } else {
            arity_matches
                .iter()
                .copied()
                .filter(|candidate| {
                    candidate
                        .parameters
                        .iter()
                        .zip(arguments)
                        .all(|(expected, actual)| {
                            actual
                                .as_deref()
                                .is_none_or(|actual| self.is_assignable(expected, actual))
                        })
                })
                .collect::<Vec<_>>()
        };
        let matching = if matching.len() > 1 {
            let mut ranked = matching
                .into_iter()
                .filter_map(|candidate| {
                    overload_score(candidate, arguments).map(|score| (candidate, score))
                })
                .collect::<Vec<_>>();
            if let Some(best_score) = ranked.iter().map(|(_, score)| *score).min() {
                ranked.retain(|(_, score)| *score == best_score);
            }
            ranked
                .into_iter()
                .map(|(candidate, _)| candidate)
                .collect::<Vec<_>>()
        } else {
            matching
        };
        if matching.len() == 1 {
            return Some(matching[0].result.clone());
        }
        if matching.len() > 1 {
            self.emit(
                "NX1107",
                format!("Der Aufruf von '{display_name}' ist mehrdeutig."),
                span,
            );
        } else {
            let signatures = overloads
                .iter()
                .map(|candidate| format!("{}({})", display_name, candidate.parameters.join(", ")))
                .collect::<Vec<_>>()
                .join("; ");
            let message = if arity_matches.is_empty() {
                format!(
                    "Keine Überladung von '{display_name}' akzeptiert {} Argumente. Kandidaten: {signatures}.",
                    arguments.len()
                )
            } else {
                format!("Keine passende Überladung für '{display_name}'. Kandidaten: {signatures}.")
            };
            self.emit("NX1107", message, span);
        }
        None
    }

    pub(super) fn argument_types(
        &mut self,
        arguments: &[Expression],
        scopes: &mut Vec<Scope>,
        owner: Option<&str>,
        is_async: bool,
    ) -> Vec<Option<String>> {
        arguments
            .iter()
            .map(|argument| {
                let inferred = self.infer(argument, scopes, owner, is_async);
                if unsuffixed_integer_literal(argument).is_some() {
                    Some("{integer}".to_string())
                } else {
                    inferred
                }
            })
            .collect()
    }

    pub(super) fn check_constructor(
        &mut self,
        class_name: &str,
        generic_arguments: &[String],
        arguments: &[Option<String>],
        span: SourceSpan,
    ) {
        let Some(class) = self.classes.get(class_name).cloned() else {
            return;
        };
        if class.constructors.is_empty() {
            if !arguments.is_empty() {
                self.emit(
                    "NX1107",
                    format!(
                        "Typ '{class_name}' besitzt keinen Konstruktor mit {} Argumenten.",
                        arguments.len()
                    ),
                    span,
                );
            }
            return;
        }
        let map = class
            .generics
            .iter()
            .cloned()
            .zip(generic_arguments.iter().cloned())
            .collect::<HashMap<_, _>>();
        let constructors = class
            .constructors
            .iter()
            .map(|constructor| substitute_function(constructor, &map))
            .collect::<Vec<_>>();
        self.resolve_overload(
            &constructors,
            arguments,
            span,
            &format!("Konstruktor von {class_name}"),
        );
    }

    pub(super) fn generic_map(&self, receiver: &str) -> HashMap<String, String> {
        let (name, arguments) =
            split_generic(receiver.trim_start_matches('&').trim_start_matches("mut "));
        match self.classes.get(name) {
            Some(class) if class.generics.len() == arguments.len() => class
                .generics
                .iter()
                .cloned()
                .zip(arguments.iter().map(|value| value.to_string()))
                .collect(),
            _ => HashMap::new(),
        }
    }

    pub(super) fn substitute_for_receiver(
        &self,
        receiver: &str,
        methods: Vec<FunctionType>,
    ) -> Vec<FunctionType> {
        let map = self.generic_map(receiver);
        if map.is_empty() {
            return methods;
        }
        methods
            .iter()
            .map(|method| substitute_function(method, &map))
            .collect()
    }

    pub(super) fn emit(&mut self, code: &str, message: impl Into<String>, span: SourceSpan) {
        let message = message.into();
        // A declaration may already have been checked by the structural
        // semantic pass. Keep one primary diagnostic per code and source span.
        if !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code && diagnostic.span == span)
        {
            self.diagnostics
                .push(Diagnostic::error_with_code(code, message, span));
        }
    }
}

use super::*;

impl Checker<'_> {
    pub(super) fn new_local_binding(&mut self, type_name: String) -> LocalBinding {
        let id = self.next_binding_id;
        self.next_binding_id += 1;
        LocalBinding {
            type_name,
            id: Some(id),
        }
    }

    pub(super) fn collect_signatures(&mut self, program: &Program) {
        self.known_types.extend(
            crate::type_system::BUILTIN_TYPE_NAMES
                .iter()
                .map(|name| (*name).to_string()),
        );
        self.known_types.extend(
            [
                "object",
                "dynamic",
                "IntList",
                "StringList",
                "RuntimeException",
                "Option",
                "Result",
                "Task",
                "Future",
                "Box",
                "Rc",
                "Arc",
                "Cell",
                "RefCell",
                "Mutex",
                "RwLock",
                "Span",
                "Memory",
                "Func",
                "Action",
                "Iterator",
            ]
            .into_iter()
            .map(str::to_string),
        );
        if program.usings.iter().any(|item| {
            let namespace = item.path.join(".");
            namespace == "System.Collections" || namespace.starts_with("System.Collections.")
        }) {
            self.known_types.extend(
                ["List", "Vec", "Dictionary", "HashSet", "Queue", "Stack"]
                    .into_iter()
                    .map(str::to_string),
            );
        }
        if program
            .usings
            .iter()
            .any(|item| item.path.join(".") == "System.IO")
        {
            self.known_types
                .extend(["File", "IoError"].into_iter().map(str::to_string));
        }
        if program
            .usings
            .iter()
            .any(|item| item.path.join(".") == "System.Text")
        {
            self.known_types.insert("TextInfo".to_string());
        }
        for declaration in &program.declarations {
            match declaration {
                Declaration::Class(class) => {
                    self.known_types.insert(class.name.clone());
                    self.known_types
                        .extend(class.generic_parameters.iter().cloned());
                    self.type_parameters
                        .extend(class.generic_parameters.iter().cloned());
                    for member in &class.members {
                        if let MemberDeclaration::Method(method) = member {
                            self.known_types
                                .extend(method.generic_parameters.iter().cloned());
                            self.type_parameters
                                .extend(method.generic_parameters.iter().cloned());
                        }
                    }
                }
                Declaration::Function(function) => {
                    self.known_types
                        .extend(function.generic_parameters.iter().cloned());
                    self.type_parameters
                        .extend(function.generic_parameters.iter().cloned());
                }
                Declaration::GlobalVariable(global) => {
                    self.global_variables
                        .insert(global.name.clone(), type_name(&global.type_ref));
                }
            }
        }
        for declaration in &program.declarations {
            match declaration {
                Declaration::Function(function) => {
                    self.functions
                        .entry(function.name.clone())
                        .or_default()
                        .push(function_type(function, None));
                }
                Declaration::Class(class) => {
                    let class_names = program
                        .declarations
                        .iter()
                        .filter_map(|declaration| match declaration {
                            Declaration::Class(declared)
                                if declared.kind == TypeDeclarationKind::Class =>
                            {
                                Some(declared.name.as_str())
                            }
                            _ => None,
                        })
                        .collect::<std::collections::HashSet<_>>();
                    let mut class_type = ClassType {
                        base: class
                            .base_types
                            .iter()
                            .find(|base| class_names.contains(base.name.as_str()))
                            .map(|base| base.name.clone()),
                        supertypes: class
                            .base_types
                            .iter()
                            .map(|base| base.name.clone())
                            .collect(),
                        generics: class.generic_parameters.clone(),
                        ..ClassType::default()
                    };
                    for member in &class.members {
                        match member {
                            MemberDeclaration::Field(field) => {
                                class_type
                                    .fields
                                    .insert(field.name.clone(), type_name(&field.type_ref));
                                if field.modifiers.iter().any(|modifier| modifier == "private") {
                                    class_type.private_fields.insert(field.name.clone());
                                }
                            }
                            MemberDeclaration::Method(method) if method.name == class.name => {
                                class_type
                                    .constructors
                                    .push(function_type(method, Some(&class.name)));
                            }
                            MemberDeclaration::Method(method) => {
                                class_type
                                    .methods
                                    .entry(method.name.clone())
                                    .or_default()
                                    .push(function_type(method, Some(&class.name)));
                            }
                            MemberDeclaration::Variant(variant) => {
                                class_type.enum_variants.insert(variant.name.clone());
                            }
                            MemberDeclaration::Operator(_) => {}
                        }
                    }
                    self.classes.insert(class.name.clone(), class_type);
                }
                Declaration::GlobalVariable(_) => {}
            }
        }
    }

    pub(super) fn check_program(&mut self, program: &Program) {
        for declaration in &program.declarations {
            match declaration {
                Declaration::GlobalVariable(global) => {
                    if let Some(initializer) = &global.initializer {
                        let expected = type_name(&global.type_ref);
                        let mut scopes = vec![HashMap::new()];
                        if let Some(actual) = self.infer(initializer, &mut scopes, None, false) {
                            self.expect_value(
                                &expected,
                                &actual,
                                initializer,
                                "Initialisierung einer globalen Variable",
                            );
                        }
                    }
                }
                Declaration::Function(function) => {
                    self.check_function(function, None, &HashMap::new());
                }
                Declaration::Class(class) => {
                    let mut fields = HashMap::new();
                    self.collect_fields(&class.name, &class.name, &mut fields, 0);
                    for member in &class.members {
                        if let MemberDeclaration::Field(field) = member
                            && let Some(initializer) = &field.initializer
                        {
                            let mut scopes = vec![field_scope(&fields)];
                            if let Some(actual) =
                                self.infer(initializer, &mut scopes, Some(&class.name), false)
                            {
                                self.expect_value(
                                    &type_name(&field.type_ref),
                                    &actual,
                                    initializer,
                                    "Feldinitialisierung",
                                );
                            }
                        }
                        if let MemberDeclaration::Method(method) = member
                            && method.body.is_some()
                        {
                            self.check_function(method, Some(&class.name), &fields);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn collect_fields(
        &self,
        class_name: &str,
        accessing_type: &str,
        output: &mut HashMap<String, String>,
        depth: usize,
    ) {
        if depth > self.classes.len() {
            return;
        }
        let Some(class) = self.classes.get(class_name) else {
            return;
        };
        if let Some(base) = &class.base {
            self.collect_fields(base, accessing_type, output, depth + 1);
        }
        output.extend(
            class
                .fields
                .iter()
                .filter(|(name, _)| {
                    !class.private_fields.contains(*name) || class_name == accessing_type
                })
                .map(|(name, ty)| (name.clone(), ty.clone())),
        );
    }

    pub(super) fn check_function(
        &mut self,
        function: &FunctionDeclaration,
        owner: Option<&str>,
        fields: &HashMap<String, String>,
    ) {
        self.moved_values.clear();
        let Some(body) = &function.body else {
            return;
        };
        let mut root = field_scope(fields);
        if let Some(owner) = owner {
            root.insert(
                "this".to_string(),
                LocalBinding {
                    type_name: owner.to_string(),
                    id: None,
                },
            );
        }
        for parameter in &function.parameters {
            let binding = self.new_local_binding(type_name(&parameter.type_ref));
            root.insert(parameter.name.clone(), binding);
        }
        let mut scopes = vec![root];
        let expected_return = if function
            .modifiers
            .iter()
            .any(|modifier| modifier == "async")
            && function.return_type.name == "Task"
            && function.return_type.generic_arguments.len() == 1
        {
            type_name(&function.return_type.generic_arguments[0])
        } else {
            type_name(&function.return_type)
        };
        let is_async = function
            .modifiers
            .iter()
            .any(|modifier| modifier == "async");
        self.check_block(body, &mut scopes, owner, &expected_return, is_async);
    }

    pub(super) fn check_block(
        &mut self,
        block: &BlockStatement,
        scopes: &mut Vec<Scope>,
        owner: Option<&str>,
        expected_return: &str,
        is_async: bool,
    ) {
        scopes.push(HashMap::new());
        for statement in &block.statements {
            self.check_statement(statement, scopes, owner, expected_return, is_async);
        }
        scopes.pop();
    }

    pub(super) fn check_statement(
        &mut self,
        statement: &Statement,
        scopes: &mut Vec<Scope>,
        owner: Option<&str>,
        expected_return: &str,
        is_async: bool,
    ) {
        match statement {
            Statement::Block(block) => {
                self.check_block(block, scopes, owner, expected_return, is_async)
            }
            Statement::Variable(variable) => {
                if let Some(type_ref) = &variable.type_ref {
                    self.validate_type_reference(type_ref);
                }
                let inferred = variable
                    .initializer
                    .as_ref()
                    .and_then(|value| self.infer(value, scopes, owner, is_async));
                if let (Some(initializer), Some(actual)) =
                    (variable.initializer.as_ref(), inferred.as_deref())
                {
                    self.move_value_expression(initializer, actual, scopes);
                }
                let declared = variable.type_ref.as_ref().map(type_name);
                // `void x = ...` is already reported by the structural pass;
                // a second initializer mismatch would be a follow-up error.
                if let (Some(expected), Some(actual), Some(initializer)) = (
                    declared.as_deref().filter(|ty| *ty != "void"),
                    inferred.as_deref(),
                    variable.initializer.as_ref(),
                ) {
                    self.expect_value(expected, actual, initializer, "Variableninitialisierung");
                }
                let variable_type = declared
                    .or(inferred)
                    .unwrap_or_else(|| "<unknown>".to_string());
                let binding = self.new_local_binding(variable_type);
                if let Some(scope) = scopes.last_mut() {
                    scope.insert(variable.name.clone(), binding);
                }
            }
            Statement::Return(value, span) => {
                if let Some(value) = value {
                    if let Some(actual) = self.infer(value, scopes, owner, is_async) {
                        self.expect_value(expected_return, &actual, value, "Rückgabewert");
                        self.move_value_expression(value, expected_return, scopes);
                    }
                } else if expected_return != "void" && expected_return != "!" {
                    self.emit("NX1103", "Eine Funktion mit Rückgabewert benötigt an dieser Stelle einen return-Ausdruck.", *span);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expect_expression(condition, "bool", scopes, owner, is_async, "if-Bedingung");
                let before = self.moved_values.clone();
                self.check_statement(then_branch, scopes, owner, expected_return, is_async);
                let then_moves = self.moved_values.clone();
                self.moved_values.clone_from(&before);
                if let Some(other) = else_branch {
                    self.check_statement(other, scopes, owner, expected_return, is_async);
                }
                let else_moves = self.moved_values.clone();
                self.moved_values = then_moves.union(&else_moves).cloned().collect();
            }
            Statement::While {
                condition, body, ..
            } => {
                self.expect_expression(
                    condition,
                    "bool",
                    scopes,
                    owner,
                    is_async,
                    "while-Bedingung",
                );
                let before = self.moved_values.clone();
                self.check_statement(body, scopes, owner, expected_return, is_async);
                self.moved_values.extend(before);
            }
            Statement::For {
                initializer,
                condition,
                increment,
                body,
                ..
            } => {
                scopes.push(HashMap::new());
                if let Some(initializer) = initializer {
                    self.check_statement(initializer, scopes, owner, expected_return, is_async);
                }
                if let Some(condition) = condition {
                    self.expect_expression(
                        condition,
                        "bool",
                        scopes,
                        owner,
                        is_async,
                        "for-Bedingung",
                    );
                }
                if let Some(increment) = increment {
                    self.infer(increment, scopes, owner, is_async);
                }
                let before_body = self.moved_values.clone();
                self.check_statement(body, scopes, owner, expected_return, is_async);
                self.moved_values.extend(before_body);
                scopes.pop();
            }
            Statement::ForEach {
                variable_type,
                variable_name,
                iterable,
                body,
                ..
            } => {
                self.validate_type_reference(variable_type);
                let iterable_type = self.infer(iterable, scopes, owner, is_async);
                let element_type = iterable_type.as_deref().and_then(iterator_element_type);
                if let Some(element_type) = element_type {
                    self.expect_type(
                        &type_name(variable_type),
                        element_type,
                        variable_type.span,
                        "foreach-Elementtyp",
                    );
                } else if iterable_type.is_some() {
                    self.emit("NX1107", "Der Ausdruck ist nicht iterierbar. Erwartet wird ein Array oder Iterator<T>.", iterable.span());
                }
                let binding = self.new_local_binding(type_name(variable_type));
                scopes.push(HashMap::from([(variable_name.clone(), binding)]));
                let before_body = self.moved_values.clone();
                self.check_statement(body, scopes, owner, expected_return, is_async);
                self.moved_values.extend(before_body);
                scopes.pop();
            }
            Statement::Switch { value, cases, .. } => {
                let value_type = self.infer(value, scopes, owner, is_async);
                let before = self.moved_values.clone();
                let mut branch_moves = Vec::new();
                let mut has_default = false;
                for case in cases {
                    has_default |= case.value.is_none();
                    self.moved_values.clone_from(&before);
                    if let (Some(case_value), Some(expected)) = (&case.value, value_type.as_deref())
                    {
                        self.expect_expression(
                            case_value,
                            expected,
                            scopes,
                            owner,
                            is_async,
                            "switch-Fall",
                        );
                    }
                    scopes.push(HashMap::new());
                    for child in &case.statements {
                        self.check_statement(child, scopes, owner, expected_return, is_async);
                    }
                    scopes.pop();
                    branch_moves.push(self.moved_values.clone());
                }
                if !has_default {
                    branch_moves.push(before);
                }
                self.moved_values = branch_moves.into_iter().flatten().collect();
            }
            Statement::Throw(value, _) => {
                if let Some(value) = value {
                    self.infer(value, scopes, owner, is_async);
                }
            }
            Statement::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                let before = self.moved_values.clone();
                let mut branch_moves = Vec::new();
                self.check_block(body, scopes, owner, expected_return, is_async);
                branch_moves.push(self.moved_values.clone());
                for clause in catches {
                    self.moved_values.clone_from(&before);
                    scopes.push(HashMap::new());
                    if let (Some(name), Some(ty)) = (&clause.variable_name, &clause.exception_type)
                    {
                        let caught = self.new_local_binding(type_name(ty));
                        scopes
                            .last_mut()
                            .expect("scope was pushed")
                            .insert(name.clone(), caught);
                    }
                    self.check_block(&clause.body, scopes, owner, expected_return, is_async);
                    scopes.pop();
                    branch_moves.push(self.moved_values.clone());
                }
                self.moved_values = branch_moves.into_iter().flatten().collect();
                if let Some(finally_block) = finally_block {
                    self.check_block(finally_block, scopes, owner, expected_return, is_async);
                }
            }
            Statement::Match { value, arms, .. } => {
                let value_type = self.infer(value, scopes, owner, is_async);
                if let Some(value_type) = value_type.as_deref() {
                    self.check_match_exhaustiveness(value_type, arms, value.span());
                }
                let before = self.moved_values.clone();
                let mut branch_moves = Vec::new();
                let exhaustive = arms.iter().any(|arm| arm.variant == "_");
                for arm in arms {
                    self.moved_values.clone_from(&before);
                    scopes.push(HashMap::new());
                    if let Some(name) = &arm.binding {
                        let binding = self.new_local_binding("dynamic".to_string());
                        scopes
                            .last_mut()
                            .expect("scope was pushed")
                            .insert(name.clone(), binding);
                    }
                    self.check_block(&arm.body, scopes, owner, expected_return, is_async);
                    scopes.pop();
                    branch_moves.push(self.moved_values.clone());
                }
                if !exhaustive {
                    branch_moves.push(before);
                }
                self.moved_values = branch_moves.into_iter().flatten().collect();
            }
            Statement::Expression(expression, _) => {
                if let Some(result_type) = self.infer(expression, scopes, owner, is_async)
                    && result_type.starts_with("Result<")
                {
                    self.emit(
                        "NX1108",
                        "Das Result-Ergebnis wird verworfen; behandle Ok und Err explizit.",
                        expression.span(),
                    );
                }
            }
            Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
        }
    }

    pub(super) fn check_match_exhaustiveness(
        &mut self,
        value_type: &str,
        arms: &[MatchArm],
        span: SourceSpan,
    ) {
        let class_name = nominal_type_name(value_type);
        let Some(class) = self.classes.get(class_name) else {
            return;
        };
        if class.enum_variants.is_empty() || arms.iter().any(|arm| arm.variant == "_") {
            return;
        }
        let covered = arms
            .iter()
            .map(|arm| {
                arm.variant
                    .strip_prefix(&format!("{class_name}."))
                    .unwrap_or(&arm.variant)
            })
            .collect::<std::collections::HashSet<_>>();
        let missing = class
            .enum_variants
            .iter()
            .filter(|variant| !covered.contains(variant.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.emit(
                "NX1109",
                format!(
                    "Der match-Ausdruck ist nicht vollständig; fehlende Varianten: {}.",
                    missing.join(", ")
                ),
                span,
            );
        }
    }
}

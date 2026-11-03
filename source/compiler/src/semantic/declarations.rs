use super::*;

pub(super) fn validate_free_function_declaration(
    function: &FunctionDeclaration,
    allow_intrinsics: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_duplicate_modifiers(&function.modifiers, function.span, diagnostics);
    if function.modifiers.iter().any(|modifier| modifier == "mut") {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "'mut' kennzeichnet Variablen, nicht Funktionen.",
            function.span,
        ));
    }
    let is_abstract = function
        .modifiers
        .iter()
        .any(|modifier| modifier == "abstract");
    let is_extern = function
        .modifiers
        .iter()
        .any(|modifier| modifier == "extern");
    let is_intrinsic = function
        .modifiers
        .iter()
        .any(|modifier| modifier == "intrinsic");
    let declaration_modifiers = [is_abstract, is_extern, is_intrinsic]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();

    if declaration_modifiers > 1 {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "abstract, extern und intrinsic schließen sich gegenseitig aus.",
            function.span,
        ));
    }
    if is_abstract {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "Freie Funktionen dürfen nicht abstract sein.",
            function.span,
        ));
    }
    if is_extern {
        let abi = function
            .modifiers
            .iter()
            .find_map(|modifier| modifier.strip_prefix("abi:"));
        match abi {
            Some("C") => {}
            Some(other) => diagnostics.push(Diagnostic::error_with_code(
                "NX1005",
                format!("Die ABI \"{other}\" wird nicht unterstützt; verwende extern \"C\"."),
                function.span,
            )),
            None => diagnostics.push(Diagnostic::error_with_code(
                "NX1005",
                "extern-Deklarationen benötigen eine explizite ABI-Angabe: extern \"C\".",
                function.span,
            )),
        }
    }
    if is_intrinsic && !allow_intrinsics {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "intrinsic ist ausschließlich in compilerverwalteten Dateien der NORTEX-Standardbibliothek zulässig.",
            function.span,
        ));
    }
    if is_intrinsic
        && function
            .modifiers
            .iter()
            .any(|modifier| modifier == "async")
    {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "Eine intrinsic-Funktion darf in dieser Sprachversion nicht async sein.",
            function.span,
        ));
    }
    if function.body.is_none() && !is_extern && !is_intrinsic {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            format!(
                "Funktion '{}' benötigt einen Implementierungsblock.",
                function.name
            ),
            function.span,
        ));
    }
    if function.body.is_some() && (is_extern || is_intrinsic || is_abstract) {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "abstract-, extern- und intrinsic-Funktionen dürfen keinen Implementierungsblock besitzen.",
            function.span,
        ));
    }
}

pub(super) fn validate_global_variable(
    global: &FieldDeclaration,
    known_types: &HashMap<String, KnownType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_duplicate_modifiers(&global.modifiers, global.span, diagnostics);
    let is_mutable = global.modifiers.iter().any(|modifier| modifier == "mut");
    let is_constant = global.modifiers.iter().any(|modifier| modifier == "const");
    if is_mutable && is_constant {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "Eine globale Variable kann nicht gleichzeitig mut und const sein.",
            global.span,
        ));
    }
    validate_type(&global.type_ref, known_types, &HashSet::new(), diagnostics);
    if is_constant {
        validate_constant_name(&global.name, global.span, diagnostics);
    } else {
        validate_value_name(&global.name, global.span, diagnostics);
    }
    if global.initializer.is_none() {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "Eine globale Variable benötigt einen Initialisierungswert.",
            global.span,
        ));
    } else if !global
        .initializer
        .as_ref()
        .is_some_and(is_constant_expression)
    {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "Der Initialisierungswert einer globalen Variable muss zur Compile-Zeit auswertbar sein.",
            global.span,
        ));
    }
}

pub(super) fn validate_using_imports(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    const STANDARD_NAMESPACES: &[&str] = &[
        "System",
        "System.Collections",
        "System.Collections.Generic",
        "System.IO",
        "System.Text",
        "System.Math",
        "System.Threading",
        "System.Time",
        "System.Net",
        "System.Memory",
        "System.Process",
        "System.Debug",
        "System.Environment",
    ];
    let own_namespace = program.namespace.as_ref().map(|parts| parts.join("."));
    for import in &program.usings {
        let path = import.path.join(".");
        let is_standard = STANDARD_NAMESPACES.contains(&path.as_str());
        let is_own_module = own_namespace.as_deref() == Some(path.as_str());
        if !is_standard && !is_own_module {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1003",
                format!("Namespace '{path}' kann nicht aufgelöst werden. Der Import ist weder Teil der NORTEX-Standardbibliothek noch im aktuellen Modul deklariert."),
                import.span,
            ).with_help("Prüfe den Namespace-Pfad und die Schreibweise."));
        }
    }
}

pub(super) fn validate_duplicate_modifiers(
    modifiers: &[String],
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();
    for modifier in modifiers {
        if !seen.insert(modifier) {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1005",
                format!("Der Modifikator '{modifier}' darf nicht mehrfach angegeben werden."),
                span,
            ));
        }
    }
}

pub(super) fn validate_abstract_contracts(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let classes: HashMap<&str, &ClassDeclaration> = program
        .declarations
        .iter()
        .filter_map(|declaration| {
            if let Declaration::Class(class) = declaration {
                Some((class.name.as_str(), class))
            } else {
                None
            }
        })
        .collect();

    fn collect_requirements(
        class: &ClassDeclaration,
        classes: &HashMap<&str, &ClassDeclaration>,
        visited: &mut HashSet<String>,
        requirements: &mut HashMap<String, (String, SourceSpan)>,
    ) {
        if !visited.insert(class.name.clone()) {
            return;
        }
        for base in &class.base_types {
            if let Some(parent) = classes.get(base.name.as_str()) {
                collect_requirements(parent, classes, visited, requirements);
            }
        }
        for member in &class.members {
            if let MemberDeclaration::Method(method) = member {
                let signature = function_contract_signature(method);
                if method
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "abstract")
                    || (matches!(
                        class.kind,
                        TypeDeclarationKind::Interface | TypeDeclarationKind::Trait
                    ) && method.body.is_none())
                {
                    requirements.insert(signature, (method.name.clone(), method.span));
                }
            }
        }
    }

    fn remove_implemented_contracts(
        class: &ClassDeclaration,
        classes: &HashMap<&str, &ClassDeclaration>,
        visited: &mut HashSet<String>,
        requirements: &mut HashMap<String, (String, SourceSpan)>,
    ) {
        if !visited.insert(class.name.clone()) {
            return;
        }
        for base in &class.base_types {
            if let Some(parent) = classes.get(base.name.as_str()) {
                remove_implemented_contracts(parent, classes, visited, requirements);
            }
        }
        for member in &class.members {
            if let MemberDeclaration::Method(method) = member
                && method.body.is_some()
            {
                requirements.remove(&function_contract_signature(method));
            }
        }
    }

    fn find_inherited_method<'a>(
        class: &'a ClassDeclaration,
        signature: &str,
        classes: &HashMap<&'a str, &'a ClassDeclaration>,
        visited: &mut HashSet<String>,
    ) -> Option<&'a FunctionDeclaration> {
        if !visited.insert(class.name.clone()) {
            return None;
        }
        for base in &class.base_types {
            let Some(parent) = classes.get(base.name.as_str()).copied() else {
                continue;
            };
            if let Some(method) = parent.members.iter().find_map(|member| {
                if let MemberDeclaration::Method(candidate) = member {
                    (function_contract_signature(candidate) == signature).then_some(candidate)
                } else {
                    None
                }
            }) {
                return Some(method);
            }
            if let Some(method) = find_inherited_method(parent, signature, classes, visited) {
                return Some(method);
            }
        }
        None
    }

    for class in classes
        .values()
        .filter(|class| class.kind == TypeDeclarationKind::Class)
    {
        let is_abstract = class
            .modifiers
            .iter()
            .any(|modifier| modifier == "abstract");
        if !is_abstract {
            let mut requirements = HashMap::new();
            collect_requirements(class, &classes, &mut HashSet::new(), &mut requirements);
            remove_implemented_contracts(class, &classes, &mut HashSet::new(), &mut requirements);
            for (_, (method_name, base_span)) in requirements {
                diagnostics.push(
                    Diagnostic::error_with_code(
                        "NX1005",
                        format!(
                            "Konkrete Klasse '{}' implementiert die abstrakte Methode '{}' nicht.",
                            class.name, method_name
                        ),
                        class.span,
                    )
                    .with_help(format!(
                        "Die abstrakte Methode ist hier deklariert: Zeile {}.",
                        base_span.start.line
                    )),
                );
            }
        }

        for member in &class.members {
            let MemberDeclaration::Method(method) = member else {
                continue;
            };
            let signature = function_contract_signature(method);
            let base_method =
                find_inherited_method(class, &signature, &classes, &mut HashSet::new());
            let overrides_virtual = base_method.is_some_and(|candidate| {
                candidate
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "virtual" || modifier == "abstract")
            });
            let has_override = method
                .modifiers
                .iter()
                .any(|modifier| modifier == "override");
            if has_override && !overrides_virtual {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1005", format!("Methode '{}' ist als override markiert, überschreibt aber keine virtual- oder abstract-Methode.", method.name), method.span));
            } else if overrides_virtual && !has_override {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1005", format!("Methode '{}' überschreibt ein virtuelles Member und muss override deklarieren.", method.name), method.span));
            }
        }
    }

    let abstract_types: HashSet<&str> = classes
        .values()
        .filter(|class| {
            class.kind == TypeDeclarationKind::Class
                && class
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "abstract")
        })
        .map(|class| class.name.as_str())
        .collect();

    fn visit_expression(
        expression: &Expression,
        abstract_types: &HashSet<&str>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match expression {
            Expression::ObjectCreation {
                type_ref,
                arguments,
                span,
            } => {
                if abstract_types.contains(type_ref.name.as_str()) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        format!(
                            "Abstrakte Klasse '{}' kann nicht instanziiert werden.",
                            type_ref.name
                        ),
                        *span,
                    ));
                }
                for argument in arguments {
                    visit_expression(argument, abstract_types, diagnostics);
                }
            }
            Expression::Unary { operand, .. } | Expression::Group(operand, _) => {
                visit_expression(operand, abstract_types, diagnostics)
            }
            Expression::Binary { left, right, .. } => {
                visit_expression(left, abstract_types, diagnostics);
                visit_expression(right, abstract_types, diagnostics);
            }
            Expression::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                visit_expression(condition, abstract_types, diagnostics);
                visit_expression(when_true, abstract_types, diagnostics);
                visit_expression(when_false, abstract_types, diagnostics);
            }
            Expression::Assignment { target, value, .. } => {
                visit_expression(target, abstract_types, diagnostics);
                visit_expression(value, abstract_types, diagnostics);
            }
            Expression::Call {
                callee, arguments, ..
            } => {
                visit_expression(callee, abstract_types, diagnostics);
                for argument in arguments {
                    visit_expression(argument, abstract_types, diagnostics);
                }
            }
            Expression::Member { object, .. } => {
                visit_expression(object, abstract_types, diagnostics)
            }
            Expression::Index { object, index, .. } => {
                visit_expression(object, abstract_types, diagnostics);
                visit_expression(index, abstract_types, diagnostics);
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

    fn visit_statement(
        statement: &Statement,
        abstract_types: &HashSet<&str>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match statement {
            Statement::Block(block) => {
                for child in &block.statements {
                    visit_statement(child, abstract_types, diagnostics);
                }
            }
            Statement::Variable(variable) => {
                if let Some(value) = &variable.initializer {
                    visit_expression(value, abstract_types, diagnostics);
                }
            }
            Statement::Return(value, _) | Statement::Throw(value, _) => {
                if let Some(value) = value {
                    visit_expression(value, abstract_types, diagnostics);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                visit_expression(condition, abstract_types, diagnostics);
                visit_statement(then_branch, abstract_types, diagnostics);
                if let Some(other) = else_branch {
                    visit_statement(other, abstract_types, diagnostics);
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                visit_expression(condition, abstract_types, diagnostics);
                visit_statement(body, abstract_types, diagnostics);
            }
            Statement::For {
                initializer,
                condition,
                increment,
                body,
                ..
            } => {
                if let Some(initializer) = initializer {
                    visit_statement(initializer, abstract_types, diagnostics);
                }
                if let Some(condition) = condition {
                    visit_expression(condition, abstract_types, diagnostics);
                }
                if let Some(increment) = increment {
                    visit_expression(increment, abstract_types, diagnostics);
                }
                visit_statement(body, abstract_types, diagnostics);
            }
            Statement::ForEach { iterable, body, .. } => {
                visit_expression(iterable, abstract_types, diagnostics);
                visit_statement(body, abstract_types, diagnostics);
            }
            Statement::Switch { value, cases, .. } => {
                visit_expression(value, abstract_types, diagnostics);
                for case in cases {
                    if let Some(value) = &case.value {
                        visit_expression(value, abstract_types, diagnostics);
                    }
                    for child in &case.statements {
                        visit_statement(child, abstract_types, diagnostics);
                    }
                }
            }
            Statement::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                for child in &body.statements {
                    visit_statement(child, abstract_types, diagnostics);
                }
                for clause in catches {
                    for child in &clause.body.statements {
                        visit_statement(child, abstract_types, diagnostics);
                    }
                }
                if let Some(block) = finally_block {
                    for child in &block.statements {
                        visit_statement(child, abstract_types, diagnostics);
                    }
                }
            }
            Statement::Match { value, arms, .. } => {
                visit_expression(value, abstract_types, diagnostics);
                for arm in arms {
                    for child in &arm.body.statements {
                        visit_statement(child, abstract_types, diagnostics);
                    }
                }
            }
            Statement::Expression(value, _) => visit_expression(value, abstract_types, diagnostics),
            Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
        }
    }

    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                if let Some(body) = &function.body {
                    for statement in &body.statements {
                        visit_statement(statement, &abstract_types, diagnostics);
                    }
                }
            }
            Declaration::Class(class) => {
                for member in &class.members {
                    match member {
                        MemberDeclaration::Field(field) => {
                            if let Some(value) = &field.initializer {
                                visit_expression(value, &abstract_types, diagnostics);
                            }
                        }
                        MemberDeclaration::Method(method) => {
                            if let Some(body) = &method.body {
                                for statement in &body.statements {
                                    visit_statement(statement, &abstract_types, diagnostics);
                                }
                            }
                        }
                        MemberDeclaration::Operator(operator) => {
                            if let Some(body) = &operator.body {
                                for statement in &body.statements {
                                    visit_statement(statement, &abstract_types, diagnostics);
                                }
                            }
                        }
                        MemberDeclaration::Variant(_) => {}
                    }
                }
            }
            Declaration::GlobalVariable(global) => {
                if let Some(initializer) = &global.initializer {
                    visit_expression(initializer, &abstract_types, diagnostics);
                }
            }
        }
    }
}

pub(super) fn analyze_class(
    class: &ClassDeclaration,
    known_types: &HashMap<String, KnownType>,
    declared_kinds: &HashMap<String, TypeDeclarationKind>,
    class_declarations: &HashMap<&str, &ClassDeclaration>,
    allow_intrinsics: bool,
    global_values: GlobalValueContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_duplicate_modifiers(&class.modifiers, class.span, diagnostics);
    if class.modifiers.iter().any(|modifier| modifier == "mut") {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "'mut' kennzeichnet Variablen, nicht Typdeklarationen.",
            class.span,
        ));
    }
    if class
        .modifiers
        .iter()
        .any(|modifier| modifier == "abstract")
        && class.kind != TypeDeclarationKind::Class
    {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1005",
            "Nur Klassen dürfen als abstract deklariert werden.",
            class.span,
        ));
    }
    let generic_names: HashSet<String> = class.generic_parameters.iter().cloned().collect();
    let mut member_names = HashSet::new();
    let mut signatures = HashSet::new();
    let mut fields = HashSet::new();
    let mut immutable_fields = HashSet::new();
    for member in &class.members {
        let (modifiers, span) = match member {
            MemberDeclaration::Field(field) => (&field.modifiers, field.span),
            MemberDeclaration::Method(method) => (&method.modifiers, method.span),
            MemberDeclaration::Operator(operator) => (&operator.modifiers, operator.span),
            MemberDeclaration::Variant(_) => continue,
        };
        if modifiers.iter().any(|modifier| modifier == "mut") {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1005",
                "'mut' kennzeichnet lokale oder globale Variablen, nicht Klassenmember.",
                span,
            ));
        }
        if let MemberDeclaration::Field(field) = member {
            fields.insert(field.name.clone());
            if field.modifiers.iter().any(|modifier| modifier == "const") {
                immutable_fields.insert(field.name.clone());
            }
        }
    }
    // Inherited non-private fields are visible by simple name in subclasses.
    // The walk is bounded by the number of declarations to tolerate cycles.
    let mut ancestor = class
        .base_types
        .iter()
        .find(|base| declared_kinds.get(&base.name) == Some(&TypeDeclarationKind::Class))
        .map(|base| base.name.clone());
    let mut steps = 0usize;
    while let Some(name) = ancestor.take() {
        steps += 1;
        if steps > class_declarations.len() || name == class.name {
            break;
        }
        let Some(base_class) = class_declarations.get(name.as_str()) else {
            break;
        };
        for member in &base_class.members {
            if let MemberDeclaration::Field(field) = member {
                if field.modifiers.iter().any(|modifier| modifier == "private") {
                    continue;
                }
                if fields.insert(field.name.clone())
                    && field.modifiers.iter().any(|modifier| modifier == "const")
                {
                    immutable_fields.insert(field.name.clone());
                }
            }
        }
        ancestor = base_class
            .base_types
            .iter()
            .find(|base| declared_kinds.get(&base.name) == Some(&TypeDeclarationKind::Class))
            .map(|base| base.name.clone());
    }

    let mut class_generic_names = HashSet::new();
    for generic in &class.generic_parameters {
        if !class_generic_names.insert(generic.clone()) {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1001",
                format!("Generischer Parameter '{generic}' ist mehrfach definiert."),
                class.span,
            ));
        }
        validate_identifier(generic, "Generischer Parameter", class.span, diagnostics);
    }
    let mut base_class_count = 0usize;
    for base in &class.base_types {
        validate_type(base, known_types, &generic_names, diagnostics);
        let base_kind = declared_kinds.get(&base.name).copied();
        if base_kind == Some(TypeDeclarationKind::Class) {
            base_class_count += 1;
            if class.kind != TypeDeclarationKind::Class {
                diagnostics.push(Diagnostic::error(
                    "Nur Klassen dürfen von einer Basisklasse erben; Structs, Records, Interfaces und Traits dürfen keine Klasse erweitern.", base.span));
            }
        }
        if matches!(
            base_kind,
            Some(
                TypeDeclarationKind::Struct
                    | TypeDeclarationKind::Record
                    | TypeDeclarationKind::Enum
            )
        ) {
            diagnostics.push(Diagnostic::error(
                "Ein Struct, Record oder Enum kann nicht als Basisklasse oder Implementierungsvertrag verwendet werden.", base.span));
        }
    }
    if base_class_count > 1 {
        diagnostics.push(Diagnostic::error_with_code(
            "NX1002",
            "Mehrfachvererbung von Klassen ist nicht zulässig. Eine Klasse darf höchstens eine Basisklasse besitzen.",
            class.span,
        ));
    }

    for member in &class.members {
        match member {
            MemberDeclaration::Field(field) => {
                if class.kind == TypeDeclarationKind::Interface {
                    diagnostics.push(Diagnostic::error("Interfaces enthalten nur Methodensignaturen und dürfen keine Felder deklarieren.", field.span));
                }
                validate_type(&field.type_ref, known_types, &generic_names, diagnostics);
                if contains_raw_pointer(&field.type_ref)
                    && !field.modifiers.iter().any(|m| m == "unsafe")
                {
                    diagnostics.push(Diagnostic::error(
                        "Raw-Pointer-Felder müssen ausdrücklich als unsafe deklariert werden.",
                        field.span,
                    ));
                }
                if !member_names.insert(field.name.clone()) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        format!("Doppeltes Klassenmember '{}'.", field.name),
                        field.span,
                    ));
                }
                if field.modifiers.iter().any(|m| m == "const") {
                    validate_constant_name(&field.name, field.span, diagnostics);
                } else {
                    validate_value_name(&field.name, field.span, diagnostics);
                }
                if field.modifiers.iter().any(|m| m == "const") {
                    match &field.initializer {
                        Some(initializer) if is_constant_expression(initializer) => {
                            analyze_expression(initializer, &HashSet::from([field.name.clone()]), diagnostics);
                        }
                        Some(_) => diagnostics.push(Diagnostic::error_with_code(
                            "NX1105", "Der Initialisierungswert eines const-Feldes muss zur Compile-Zeit auswertbar sein.", field.span)),
                        None => diagnostics.push(Diagnostic::error_with_code(
                            "NX1105", "Ein const-Feld benötigt einen Initialisierungswert.", field.span)),
                    }
                }
            }
            MemberDeclaration::Method(method) => {
                validate_duplicate_modifiers(&method.modifiers, method.span, diagnostics);
                let is_constructor = method.name == class.name;
                let abstract_method = method
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "abstract");
                let extern_method = method.modifiers.iter().any(|modifier| modifier == "extern");
                let intrinsic_method = method
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "intrinsic");
                let declaration_modifier_count = [abstract_method, extern_method, intrinsic_method]
                    .into_iter()
                    .filter(|enabled| *enabled)
                    .count();

                if class.kind == TypeDeclarationKind::Interface && method.body.is_some() {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "Interface-Methoden definieren nur Signaturen und dürfen keinen Implementierungsblock besitzen.",
                        method.span,
                    ));
                } else if !matches!(
                    class.kind,
                    TypeDeclarationKind::Interface | TypeDeclarationKind::Trait
                ) {
                    if method.body.is_none() && declaration_modifier_count == 0 {
                        diagnostics.push(Diagnostic::error_with_code(
                            "NX1005",
                            format!("Methode '{}' benötigt einen Implementierungsblock. Signaturen ohne Rumpf müssen als abstract oder extern deklariert werden; intrinsic ist nur für die Standardbibliothek zulässig.", method.name),
                            method.span,
                        ));
                    }
                    if method.body.is_some() && declaration_modifier_count > 0 {
                        diagnostics.push(Diagnostic::error_with_code(
                            "NX1005",
                            "abstract-, extern- und intrinsic-Methoden dürfen keinen Implementierungsblock besitzen.",
                            method.span,
                        ));
                    }
                }

                if extern_method {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "extern-Deklarationen benötigen eine explizite ABI-Angabe. Die Syntax extern \"C\" wird noch nicht unterstützt.",
                        method.span,
                    ));
                }
                if matches!(
                    class.kind,
                    TypeDeclarationKind::Interface | TypeDeclarationKind::Trait
                ) && (extern_method || intrinsic_method)
                {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "Interface- und Trait-Verträge dürfen weder extern noch intrinsic sein.",
                        method.span,
                    ));
                }
                if intrinsic_method && !allow_intrinsics {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "intrinsic ist ausschließlich in compilerverwalteten Dateien der NORTEX-Standardbibliothek zulässig.",
                        method.span,
                    ));
                }
                if declaration_modifier_count > 1 {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "abstract, extern und intrinsic schließen sich gegenseitig aus.",
                        method.span,
                    ));
                }
                if intrinsic_method && method.modifiers.iter().any(|modifier| modifier == "async") {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "Eine intrinsic-Methode darf in dieser Sprachversion nicht async sein.",
                        method.span,
                    ));
                }
                if is_constructor && (abstract_method || intrinsic_method || extern_method) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1005",
                        "Konstruktoren dürfen weder abstract, extern noch intrinsic sein.",
                        method.span,
                    ));
                }
                if abstract_method {
                    if class.kind != TypeDeclarationKind::Class
                        || !class
                            .modifiers
                            .iter()
                            .any(|modifier| modifier == "abstract")
                    {
                        diagnostics.push(Diagnostic::error_with_code(
                            "NX1005",
                            "Eine abstrakte Methode darf nur in einer abstract-Klasse stehen.",
                            method.span,
                        ));
                    }
                    if method
                        .modifiers
                        .iter()
                        .any(|modifier| modifier == "private" || modifier == "static")
                    {
                        diagnostics.push(Diagnostic::error_with_code(
                            "NX1005",
                            "Eine abstrakte Methode darf weder private noch static sein.",
                            method.span,
                        ));
                    }
                }
                validate_function(
                    method,
                    Some(&class.name),
                    FunctionValidationContext {
                        known_types,
                        enclosing_generics: &generic_names,
                        fields: &fields,
                        immutable_fields: &immutable_fields,
                        global_values: global_values.names,
                        immutable_globals: global_values.immutable,
                        class_declarations,
                    },
                    diagnostics,
                );
                let signature = function_signature(method);
                if !signatures.insert(signature) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        format!("Doppelte Methodensignatur '{}'.", method.name),
                        method.span,
                    ));
                }
                // Methods may be overloaded, but a method cannot reuse a field name.
                if fields.contains(&method.name) {
                    diagnostics.push(Diagnostic::error(
                        format!(
                            "Methode und Feld dürfen nicht denselben Bezeichner '{}' verwenden.",
                            method.name
                        ),
                        method.span,
                    ));
                }
            }
            MemberDeclaration::Variant(variant) => {
                if class.kind != TypeDeclarationKind::Enum {
                    diagnostics.push(Diagnostic::error(
                        "Enum-Varianten sind nur innerhalb von enum-Deklarationen zulässig.",
                        variant.span,
                    ));
                }
                if !member_names.insert(variant.name.clone()) {
                    diagnostics.push(Diagnostic::error(
                        format!("Doppelte Enum-Variante '{}'.", variant.name),
                        variant.span,
                    ));
                }
                validate_identifier(&variant.name, "Enum-Variante", variant.span, diagnostics);
                for payload_type in &variant.payload_types {
                    validate_type(payload_type, known_types, &generic_names, diagnostics);
                }
            }
            MemberDeclaration::Operator(operator) => {
                if class.kind == TypeDeclarationKind::Interface {
                    diagnostics.push(Diagnostic::error(
                        "Operatorimplementierungen sind in Interfaces nicht zulässig.",
                        operator.span,
                    ));
                }
                const ALLOWED: &[&str] = &["==", "!=", "<", ">", "<=", ">="];
                if !ALLOWED.contains(&operator.operator.as_str()) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1104",
                        format!("Der Operator '{}' darf in NORTEX nicht überladen werden. Erlaubt sind ausschließlich ==, !=, <, >, <= und >=.", operator.operator),
                        operator.span,
                    ));
                }
                if operator.parameters.len() != 2 {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1104",
                        "Ein binärer Vergleichsoperator benötigt genau zwei Parameter.",
                        operator.span,
                    ));
                }
                if !operator.modifiers.iter().any(|m| m == "static") {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1104",
                        "Eine Operatorüberladung muss als static deklariert werden.",
                        operator.span,
                    ));
                }
                let operator_signature = format!(
                    "operator {}({})",
                    operator.operator,
                    operator
                        .parameters
                        .iter()
                        .map(|p| p.type_ref.name.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                );
                if !signatures.insert(operator_signature) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        "Doppelte Operatorüberladung.",
                        operator.span,
                    ));
                }
                if operator.return_type.name != "bool" {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1104",
                        "Vergleichsoperatoren müssen bool zurückgeben.",
                        operator.return_type.span,
                    ));
                }
                validate_type(
                    &operator.return_type,
                    known_types,
                    &generic_names,
                    diagnostics,
                );
                for parameter in &operator.parameters {
                    validate_type(
                        &parameter.type_ref,
                        known_types,
                        &generic_names,
                        diagnostics,
                    );
                }
            }
        }
    }
}

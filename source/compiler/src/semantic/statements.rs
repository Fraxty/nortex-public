use super::*;

pub(super) fn analyze_block(
    block: &BlockStatement,
    visible: &mut HashSet<String>,
    immutable: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Each block owns a separate lexical scope; nested blocks are analyzed
    // with cloned visibility and mutability sets.
    for statement in &block.statements {
        match statement {
            Statement::Variable(variable) => {
                validate_variable_literal_type(variable, diagnostics);
                if variable.type_ref.is_none() && variable.initializer.is_none() {
                    diagnostics.push(Diagnostic::error("Eine Variable ohne Typ benötigt einen Initialisierungswert für die Typinferenz.", variable.span));
                }
                if !visible.insert(variable.name.clone()) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        format!(
                            "Bezeichner '{}' ist in diesem Gültigkeitsbereich bereits definiert.",
                            variable.name
                        ),
                        variable.span,
                    ));
                }
                if variable.mutability == VariableMutability::CompileTime {
                    validate_constant_name(&variable.name, variable.span, diagnostics);
                    if variable.initializer.is_none() {
                        diagnostics.push(Diagnostic::error(
                            "Eine Compile-Time-Konstante benötigt einen Initialisierungswert.",
                            variable.span,
                        ));
                    } else if !variable
                        .initializer
                        .as_ref()
                        .is_some_and(is_constant_expression)
                    {
                        diagnostics.push(Diagnostic::error("Der Initialisierungswert einer const-Konstante muss zur Compile-Zeit auswertbar sein.", variable.span));
                    }
                } else {
                    validate_value_name(&variable.name, variable.span, diagnostics);
                    if matches!(
                        variable.mutability,
                        VariableMutability::Immutable | VariableMutability::CompileTime
                    ) && variable.initializer.is_none()
                    {
                        diagnostics.push(Diagnostic::error(
                            "Eine unveränderliche Variable benötigt einen Initialisierungswert.",
                            variable.span,
                        ));
                    }
                }
                if matches!(
                    variable.mutability,
                    VariableMutability::Immutable | VariableMutability::CompileTime
                ) {
                    immutable.insert(variable.name.clone());
                }
                if let Some(value) = &variable.initializer {
                    analyze_expression(value, immutable, diagnostics);
                }
            }
            Statement::Expression(expression, _) => {
                analyze_expression(expression, immutable, diagnostics)
            }
            Statement::Return(value, _) | Statement::Throw(value, _) => {
                if let Some(value) = value {
                    analyze_expression(value, immutable, diagnostics);
                }
            }
            Statement::Block(nested) => {
                let mut child_visible = visible.clone();
                let mut child_immutable = immutable.clone();
                analyze_block(
                    nested,
                    &mut child_visible,
                    &mut child_immutable,
                    diagnostics,
                );
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                analyze_expression(condition, immutable, diagnostics);
                analyze_statement(then_branch, visible, immutable, diagnostics);
                if let Some(branch) = else_branch {
                    analyze_statement(branch, visible, immutable, diagnostics);
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                analyze_expression(condition, immutable, diagnostics);
                analyze_statement(body, visible, immutable, diagnostics);
            }
            Statement::For {
                initializer,
                condition,
                increment,
                body,
                ..
            } => {
                // The for-header has its own scope so its initializer does not
                // leak into the surrounding function block.
                let mut loop_visible = visible.clone();
                let mut loop_immutable = immutable.clone();
                if let Some(initializer) = initializer {
                    analyze_statement(
                        initializer,
                        &mut loop_visible,
                        &mut loop_immutable,
                        diagnostics,
                    );
                }
                if let Some(condition) = condition {
                    analyze_expression(condition, &loop_immutable, diagnostics);
                }
                if let Some(increment) = increment {
                    analyze_expression(increment, &loop_immutable, diagnostics);
                }
                analyze_statement(body, &mut loop_visible, &mut loop_immutable, diagnostics);
            }
            Statement::ForEach {
                variable_type: _,
                variable_name,
                iterable,
                body,
                ..
            } => {
                analyze_expression(iterable, immutable, diagnostics);
                let mut loop_visible = visible.clone();
                let mut loop_immutable = immutable.clone();
                if !loop_visible.insert(variable_name.clone()) {
                    diagnostics.push(Diagnostic::error_with_code(
                        "NX1001",
                        format!("Foreach-Variable '{variable_name}' ist bereits im Gültigkeitsbereich definiert."),
                        iterable.span(),
                    ));
                }
                validate_value_name(variable_name, iterable.span(), diagnostics);
                analyze_statement(body, &mut loop_visible, &mut loop_immutable, diagnostics);
            }
            Statement::Switch { value, cases, .. } => {
                analyze_expression(value, immutable, diagnostics);
                analyze_switch_cases(cases, visible, immutable, diagnostics);
            }
            Statement::Break(_) | Statement::Continue(_) => {}
            Statement::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                analyze_block(body, visible, immutable, diagnostics);
                for clause in catches {
                    analyze_block(&clause.body, visible, immutable, diagnostics);
                }
                if let Some(finally_body) = finally_block {
                    analyze_block(finally_body, visible, immutable, diagnostics);
                }
            }
            Statement::Match { value, arms, .. } => {
                analyze_expression(value, immutable, diagnostics);
                analyze_match_arms(arms, visible, immutable, diagnostics);
            }
            Statement::Empty(_) => {}
        }
    }
}

pub(super) fn analyze_switch_cases(
    cases: &[SwitchCase],
    visible: &HashSet<String>,
    immutable: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut default_seen = false;
    for (index, case) in cases.iter().enumerate() {
        match &case.value {
            Some(case_value) => {
                if default_seen {
                    diagnostics.push(Diagnostic::error(
                        "Ein default-Fall muss der letzte switch-Fall sein.",
                        case.span,
                    ));
                }
                analyze_expression(case_value, immutable, diagnostics);
            }
            None => {
                if default_seen {
                    diagnostics.push(Diagnostic::error(
                        "Ein switch darf höchstens einen default-Fall besitzen.",
                        case.span,
                    ));
                }
                default_seen = true;
                if index + 1 != cases.len() {
                    diagnostics.push(Diagnostic::warning(
                        "Der default-Fall sollte am Ende des switch stehen.",
                        case.span,
                    ));
                }
            }
        }
        let mut case_visible = visible.clone();
        let mut case_immutable = immutable.clone();
        for statement in &case.statements {
            analyze_statement(
                statement,
                &mut case_visible,
                &mut case_immutable,
                diagnostics,
            );
        }
    }
}

pub(super) fn analyze_statement(
    statement: &Statement,
    visible: &mut HashSet<String>,
    immutable: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Block(block) => {
            let mut child_visible = visible.clone();
            let mut child_immutable = immutable.clone();
            analyze_block(block, &mut child_visible, &mut child_immutable, diagnostics);
        }
        Statement::Variable(variable) => {
            validate_variable_literal_type(variable, diagnostics);
            if !visible.insert(variable.name.clone()) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1001",
                    format!(
                        "Bezeichner '{}' ist in diesem Gültigkeitsbereich bereits definiert.",
                        variable.name
                    ),
                    variable.span,
                ));
            }
            if variable.mutability == VariableMutability::CompileTime {
                validate_constant_name(&variable.name, variable.span, diagnostics);
                if variable.initializer.is_none() {
                    diagnostics.push(Diagnostic::error(
                        "Eine Compile-Time-Konstante benötigt einen Initialisierungswert.",
                        variable.span,
                    ));
                } else if !variable
                    .initializer
                    .as_ref()
                    .is_some_and(is_constant_expression)
                {
                    diagnostics.push(Diagnostic::error("Der Initialisierungswert einer const-Konstante muss zur Compile-Zeit auswertbar sein.", variable.span));
                }
            } else {
                validate_value_name(&variable.name, variable.span, diagnostics);
                if matches!(
                    variable.mutability,
                    VariableMutability::Immutable | VariableMutability::CompileTime
                ) && variable.initializer.is_none()
                {
                    diagnostics.push(Diagnostic::error(
                        "Eine unveränderliche Variable benötigt einen Initialisierungswert.",
                        variable.span,
                    ));
                }
            }
            if matches!(
                variable.mutability,
                VariableMutability::Immutable | VariableMutability::CompileTime
            ) {
                immutable.insert(variable.name.clone());
            }
            if let Some(value) = &variable.initializer {
                analyze_expression(value, immutable, diagnostics);
            }
        }
        Statement::Expression(expression, _) => {
            analyze_expression(expression, immutable, diagnostics)
        }
        Statement::Return(value, _) | Statement::Throw(value, _) => {
            if let Some(value) = value {
                analyze_expression(value, immutable, diagnostics);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            analyze_expression(condition, immutable, diagnostics);
            analyze_statement(then_branch, visible, immutable, diagnostics);
            if let Some(other) = else_branch {
                analyze_statement(other, visible, immutable, diagnostics);
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            analyze_expression(condition, immutable, diagnostics);
            analyze_statement(body, visible, immutable, diagnostics);
        }
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            ..
        } => {
            let mut loop_visible = visible.clone();
            let mut loop_immutable = immutable.clone();
            if let Some(initializer) = initializer {
                analyze_statement(
                    initializer,
                    &mut loop_visible,
                    &mut loop_immutable,
                    diagnostics,
                );
            }
            if let Some(condition) = condition {
                analyze_expression(condition, &loop_immutable, diagnostics);
            }
            if let Some(increment) = increment {
                analyze_expression(increment, &loop_immutable, diagnostics);
            }
            analyze_statement(body, &mut loop_visible, &mut loop_immutable, diagnostics);
        }
        Statement::ForEach {
            variable_name,
            iterable,
            body,
            ..
        } => {
            analyze_expression(iterable, immutable, diagnostics);
            let mut loop_visible = visible.clone();
            let mut loop_immutable = immutable.clone();
            if !loop_visible.insert(variable_name.clone()) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1001",
                    format!("Foreach-Variable '{variable_name}' ist bereits im Gültigkeitsbereich definiert."),
                    iterable.span(),
                ));
            }
            validate_value_name(variable_name, iterable.span(), diagnostics);
            analyze_statement(body, &mut loop_visible, &mut loop_immutable, diagnostics);
        }
        Statement::Switch { value, cases, .. } => {
            analyze_expression(value, immutable, diagnostics);
            analyze_switch_cases(cases, visible, immutable, diagnostics);
        }
        Statement::Break(_) | Statement::Continue(_) => {}
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            analyze_block(body, visible, immutable, diagnostics);
            for catch in catches {
                analyze_block(&catch.body, visible, immutable, diagnostics);
            }
            if let Some(finally_body) = finally_block {
                analyze_block(finally_body, visible, immutable, diagnostics);
            }
        }
        Statement::Match { value, arms, .. } => {
            analyze_expression(value, immutable, diagnostics);
            analyze_match_arms(arms, visible, immutable, diagnostics);
        }
        Statement::Empty(_) => {}
    }
}

/// `&mut T` makes the referent mutable, not the parameter binding itself.
/// This pass therefore rejects rebinding/incrementing the reference variable;
/// member and index mutation through ordinary/shared parameters is rejected
/// by the normal immutability analysis.
pub(super) fn validate_borrowed_parameter_mutation(
    function: &FunctionDeclaration,
    body: &BlockStatement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mutable_borrows: HashSet<String> = function
        .parameters
        .iter()
        .filter(|parameter| parameter.type_ref.reference_kind == ReferenceKind::MutableBorrow)
        .map(|parameter| parameter.name.clone())
        .collect();
    validate_block_mutable_parameter_rebinding(body, &mutable_borrows, diagnostics);
}

pub(super) fn validate_block_mutable_parameter_rebinding(
    block: &BlockStatement,
    mutable_borrows: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        validate_statement_mutable_parameter_rebinding(statement, mutable_borrows, diagnostics);
    }
}

pub(super) fn validate_statement_mutable_parameter_rebinding(
    statement: &Statement,
    mutable_borrows: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Block(block) => {
            validate_block_mutable_parameter_rebinding(block, mutable_borrows, diagnostics)
        }
        Statement::Variable(variable) => {
            if let Some(initializer) = &variable.initializer {
                validate_expression_mutable_parameter_rebinding(
                    initializer,
                    mutable_borrows,
                    diagnostics,
                );
            }
        }
        Statement::Expression(expression, _) => validate_expression_mutable_parameter_rebinding(
            expression,
            mutable_borrows,
            diagnostics,
        ),
        Statement::Return(value, _) | Statement::Throw(value, _) => {
            if let Some(value) = value {
                validate_expression_mutable_parameter_rebinding(
                    value,
                    mutable_borrows,
                    diagnostics,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            validate_expression_mutable_parameter_rebinding(
                condition,
                mutable_borrows,
                diagnostics,
            );
            validate_statement_mutable_parameter_rebinding(
                then_branch,
                mutable_borrows,
                diagnostics,
            );
            if let Some(other) = else_branch {
                validate_statement_mutable_parameter_rebinding(other, mutable_borrows, diagnostics);
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            validate_expression_mutable_parameter_rebinding(
                condition,
                mutable_borrows,
                diagnostics,
            );
            validate_statement_mutable_parameter_rebinding(body, mutable_borrows, diagnostics);
        }
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                validate_statement_mutable_parameter_rebinding(
                    initializer,
                    mutable_borrows,
                    diagnostics,
                );
            }
            if let Some(condition) = condition {
                validate_expression_mutable_parameter_rebinding(
                    condition,
                    mutable_borrows,
                    diagnostics,
                );
            }
            if let Some(increment) = increment {
                validate_expression_mutable_parameter_rebinding(
                    increment,
                    mutable_borrows,
                    diagnostics,
                );
            }
            validate_statement_mutable_parameter_rebinding(body, mutable_borrows, diagnostics);
        }
        Statement::ForEach { iterable, body, .. } => {
            validate_expression_mutable_parameter_rebinding(iterable, mutable_borrows, diagnostics);
            validate_statement_mutable_parameter_rebinding(body, mutable_borrows, diagnostics);
        }
        Statement::Switch { value, cases, .. } => {
            validate_expression_mutable_parameter_rebinding(value, mutable_borrows, diagnostics);
            for case in cases {
                if let Some(value) = &case.value {
                    validate_expression_mutable_parameter_rebinding(
                        value,
                        mutable_borrows,
                        diagnostics,
                    );
                }
                for child in &case.statements {
                    validate_statement_mutable_parameter_rebinding(
                        child,
                        mutable_borrows,
                        diagnostics,
                    );
                }
            }
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            validate_block_mutable_parameter_rebinding(body, mutable_borrows, diagnostics);
            for catch in catches {
                validate_block_mutable_parameter_rebinding(
                    &catch.body,
                    mutable_borrows,
                    diagnostics,
                );
            }
            if let Some(finally_body) = finally_block {
                validate_block_mutable_parameter_rebinding(
                    finally_body,
                    mutable_borrows,
                    diagnostics,
                );
            }
        }
        Statement::Match { value, arms, .. } => {
            validate_expression_mutable_parameter_rebinding(value, mutable_borrows, diagnostics);
            for arm in arms {
                validate_block_mutable_parameter_rebinding(&arm.body, mutable_borrows, diagnostics);
            }
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
    }
}

pub(super) fn identifier_in_set<'a>(
    expression: &'a Expression,
    values: &HashSet<String>,
) -> Option<&'a str> {
    identifier_in_set_when(expression, values, true)
}

pub(super) fn identifier_in_set_when<'a>(
    expression: &'a Expression,
    values: &HashSet<String>,
    condition: bool,
) -> Option<&'a str> {
    match (condition, expression) {
        (true, Expression::Identifier(name, _)) if values.contains(name) => Some(name),
        _ => None,
    }
}

pub(super) fn validate_expression_mutable_parameter_rebinding(
    expression: &Expression,
    mutable_borrows: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expression::Assignment { target, value, span, .. } => {
            if let Some(name) = identifier_in_set(target, mutable_borrows) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1200",
                    format!("Der &mut-Parameter '{name}' ist als Bindung unveränderlich und darf nicht neu zugewiesen werden."),
                    *span,
                ));
            }
            validate_expression_mutable_parameter_rebinding(target, mutable_borrows, diagnostics);
            validate_expression_mutable_parameter_rebinding(value, mutable_borrows, diagnostics);
        }
        Expression::Unary { operator, operand, span } => {
            let is_increment = matches!(operator.as_str(), "++" | "--" | "post++" | "post--");
            if let Some(name) = identifier_in_set_when(operand, mutable_borrows, is_increment) {
                diagnostics.push(Diagnostic::error_with_code(
                    "NX1200", format!("Der &mut-Parameter '{name}' ist als Bindung unveränderlich und darf nicht inkrementiert oder dekrementiert werden."), *span));
            }
            validate_expression_mutable_parameter_rebinding(operand, mutable_borrows, diagnostics);
        }
        Expression::Group(inner, _) => validate_expression_mutable_parameter_rebinding(inner, mutable_borrows, diagnostics),
        Expression::Conditional { condition, when_true, when_false, .. } => {
            validate_expression_mutable_parameter_rebinding(condition, mutable_borrows, diagnostics);
            validate_expression_mutable_parameter_rebinding(when_true, mutable_borrows, diagnostics);
            validate_expression_mutable_parameter_rebinding(when_false, mutable_borrows, diagnostics);
        }
        Expression::Binary { left, right, .. } => {
            validate_expression_mutable_parameter_rebinding(left, mutable_borrows, diagnostics);
            validate_expression_mutable_parameter_rebinding(right, mutable_borrows, diagnostics);
        }
        Expression::Call { callee, arguments, .. } => {
            validate_expression_mutable_parameter_rebinding(callee, mutable_borrows, diagnostics);
            for argument in arguments {
                validate_expression_mutable_parameter_rebinding(argument, mutable_borrows, diagnostics);
            }
        }
        Expression::ObjectCreation { arguments, .. } => {
            for argument in arguments {
                validate_expression_mutable_parameter_rebinding(argument, mutable_borrows, diagnostics);
            }
        }
        Expression::Member { object, .. } => validate_expression_mutable_parameter_rebinding(object, mutable_borrows, diagnostics),
        Expression::Index { object, index, .. } => {
            validate_expression_mutable_parameter_rebinding(object, mutable_borrows, diagnostics);
            validate_expression_mutable_parameter_rebinding(index, mutable_borrows, diagnostics);
        }
        Expression::Null(span) => diagnostics.push(Diagnostic::error_with_code(
            "NX1102", "Das null-Literal ist in sicherem NORTEX-Code nicht zulässig. Verwende Option<T>, Some(...) oder None.", *span)),
        Expression::Identifier(_, _) | Expression::Integer(_, _) | Expression::Float(_, _) |
        Expression::String(_, _) | Expression::Character(_, _) | Expression::Boolean(_, _) => {}
    }
}

/// Validate that `break` and `continue` appear in a control-flow context
/// where their meaning is defined. A switch accepts `break`, but not
/// `continue`; loops accept both.
pub(super) fn validate_control_flow(
    block: &BlockStatement,
    loop_depth: usize,
    breakable_depth: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        validate_statement_control_flow(statement, loop_depth, breakable_depth, diagnostics);
    }
}

pub(super) fn validate_statement_control_flow(
    statement: &Statement,
    loop_depth: usize,
    breakable_depth: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Block(block) => {
            validate_control_flow(block, loop_depth, breakable_depth, diagnostics)
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            validate_statement_control_flow(then_branch, loop_depth, breakable_depth, diagnostics);
            if let Some(other) = else_branch {
                validate_statement_control_flow(other, loop_depth, breakable_depth, diagnostics);
            }
        }
        Statement::While { body, .. } => {
            validate_statement_control_flow(body, loop_depth + 1, breakable_depth + 1, diagnostics);
        }
        Statement::For { body, .. } | Statement::ForEach { body, .. } => {
            validate_statement_control_flow(body, loop_depth + 1, breakable_depth + 1, diagnostics);
        }
        Statement::Switch { cases, .. } => {
            for case in cases {
                for child in &case.statements {
                    validate_statement_control_flow(
                        child,
                        loop_depth,
                        breakable_depth + 1,
                        diagnostics,
                    );
                }
            }
        }
        Statement::Break(span) if breakable_depth == 0 => {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1301",
                "break ist nur innerhalb einer Schleife oder eines switch-Blocks zulässig.",
                *span,
            ));
        }
        Statement::Continue(span) if loop_depth == 0 => {
            diagnostics.push(Diagnostic::error_with_code(
                "NX1302",
                "continue ist nur innerhalb einer Schleife zulässig.",
                *span,
            ));
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            validate_control_flow(body, loop_depth, breakable_depth, diagnostics);
            for clause in catches {
                validate_control_flow(&clause.body, loop_depth, breakable_depth, diagnostics);
            }
            if let Some(finally_body) = finally_block {
                validate_control_flow(finally_body, loop_depth, breakable_depth, diagnostics);
            }
        }
        Statement::Match { arms, .. } => {
            for arm in arms {
                validate_control_flow(&arm.body, loop_depth, breakable_depth, diagnostics);
            }
        }
        _ => {}
    }
}

pub(super) fn validate_block_names(
    block: &BlockStatement,
    parent_scope: &HashSet<String>,
    globals: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut scope = parent_scope.clone();
    let mut declared_here = HashSet::new();
    for statement in &block.statements {
        validate_statement_names(
            statement,
            &mut scope,
            &mut declared_here,
            globals,
            diagnostics,
        );
    }
}

pub(super) fn validate_statement_names(
    statement: &Statement,
    scope: &mut HashSet<String>,
    declared_here: &mut HashSet<String>,
    globals: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Block(block) => validate_block_names(block, scope, globals, diagnostics),
        Statement::Variable(variable) => {
            if let Some(initializer) = &variable.initializer {
                validate_expression_names(initializer, scope, globals, diagnostics);
            }
            let _ = declared_here.insert(variable.name.clone());
            scope.insert(variable.name.clone());
        }
        Statement::Expression(expression, _) => {
            validate_expression_names(expression, scope, globals, diagnostics)
        }
        Statement::Return(value, _) | Statement::Throw(value, _) => {
            if let Some(value) = value {
                validate_expression_names(value, scope, globals, diagnostics);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            validate_expression_names(condition, scope, globals, diagnostics);
            let mut then_scope = scope.clone();
            let mut then_declared = HashSet::new();
            validate_statement_names(
                then_branch,
                &mut then_scope,
                &mut then_declared,
                globals,
                diagnostics,
            );
            if let Some(other) = else_branch {
                let mut else_scope = scope.clone();
                let mut else_declared = HashSet::new();
                validate_statement_names(
                    other,
                    &mut else_scope,
                    &mut else_declared,
                    globals,
                    diagnostics,
                );
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            validate_expression_names(condition, scope, globals, diagnostics);
            let mut loop_scope = scope.clone();
            let mut loop_declared = HashSet::new();
            validate_statement_names(
                body,
                &mut loop_scope,
                &mut loop_declared,
                globals,
                diagnostics,
            );
        }
        Statement::For {
            initializer,
            condition,
            increment,
            body,
            ..
        } => {
            let mut loop_scope = scope.clone();
            let mut loop_declared = HashSet::new();
            if let Some(initializer) = initializer {
                validate_statement_names(
                    initializer,
                    &mut loop_scope,
                    &mut loop_declared,
                    globals,
                    diagnostics,
                );
            }
            if let Some(condition) = condition {
                validate_expression_names(condition, &loop_scope, globals, diagnostics);
            }
            if let Some(increment) = increment {
                validate_expression_names(increment, &loop_scope, globals, diagnostics);
            }
            let mut body_scope = loop_scope;
            let mut body_declared = HashSet::new();
            validate_statement_names(
                body,
                &mut body_scope,
                &mut body_declared,
                globals,
                diagnostics,
            );
        }
        Statement::ForEach {
            variable_name,
            iterable,
            body,
            ..
        } => {
            validate_expression_names(iterable, scope, globals, diagnostics);
            let mut loop_scope = scope.clone();
            let mut loop_declared = HashSet::new();
            loop_scope.insert(variable_name.clone());
            loop_declared.insert(variable_name.clone());
            validate_statement_names(
                body,
                &mut loop_scope,
                &mut loop_declared,
                globals,
                diagnostics,
            );
        }
        Statement::Switch { value, cases, .. } => {
            validate_expression_names(value, scope, globals, diagnostics);
            for case in cases {
                let mut case_scope = scope.clone();
                let mut case_declared = HashSet::new();
                if let Some(value) = &case.value {
                    validate_expression_names(value, scope, globals, diagnostics);
                }
                for child in &case.statements {
                    validate_statement_names(
                        child,
                        &mut case_scope,
                        &mut case_declared,
                        globals,
                        diagnostics,
                    );
                }
            }
        }
        Statement::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            validate_block_names(body, scope, globals, diagnostics);
            for clause in catches {
                let mut catch_scope = scope.clone();
                if let Some(name) = &clause.variable_name {
                    catch_scope.insert(name.clone());
                }
                validate_block_names(&clause.body, &catch_scope, globals, diagnostics);
            }
            if let Some(finally_body) = finally_block {
                validate_block_names(finally_body, scope, globals, diagnostics);
            }
        }
        Statement::Match { value, arms, .. } => {
            validate_expression_names(value, scope, globals, diagnostics);
            for arm in arms {
                let mut arm_scope = scope.clone();
                if let Some(binding) = &arm.binding {
                    arm_scope.insert(binding.clone());
                }
                validate_block_names(&arm.body, &arm_scope, globals, diagnostics);
            }
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Empty(_) => {}
    }
}

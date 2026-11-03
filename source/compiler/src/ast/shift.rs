//! Verschiebt alle Quellpositionen eines Teilbaums. Der inkrementelle Parser
//! nutzt das, um unveränderte Deklarationen hinter einer Textänderung
//! wiederzuverwenden. Die `match`-Ausdrücke sind bewusst ohne Wildcard
//! geschrieben, damit neue AST-Felder hier nicht vergessen werden.

use super::*;
use crate::diagnostics::PositionShift;

impl Declaration {
    pub fn shift(&mut self, shift: &PositionShift) {
        match self {
            Declaration::Class(class) => class.shift(shift),
            Declaration::Function(function) => function.shift(shift),
            Declaration::GlobalVariable(field) => field.shift(shift),
        }
    }
}

impl UsingDirective {
    pub fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
    }
}

impl ClassDeclaration {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        for base in &mut self.base_types {
            base.shift(shift);
        }
        for member in &mut self.members {
            match member {
                MemberDeclaration::Field(field) => field.shift(shift),
                MemberDeclaration::Method(method) => method.shift(shift),
                MemberDeclaration::Operator(operator) => operator.shift(shift),
                MemberDeclaration::Variant(variant) => {
                    shift.apply_span(&mut variant.span);
                    for payload in &mut variant.payload_types {
                        payload.shift(shift);
                    }
                }
            }
        }
    }
}

impl FieldDeclaration {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        self.type_ref.shift(shift);
        if let Some(initializer) = &mut self.initializer {
            initializer.shift(shift);
        }
    }
}

impl FunctionDeclaration {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        self.return_type.shift(shift);
        for parameter in &mut self.parameters {
            parameter.shift(shift);
        }
        if let Some(body) = &mut self.body {
            body.shift(shift);
        }
    }
}

impl OperatorDeclaration {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        self.return_type.shift(shift);
        for parameter in &mut self.parameters {
            parameter.shift(shift);
        }
        if let Some(body) = &mut self.body {
            body.shift(shift);
        }
    }
}

impl Parameter {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        self.type_ref.shift(shift);
    }
}

impl TypeReference {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        for argument in &mut self.generic_arguments {
            argument.shift(shift);
        }
    }
}

impl BlockStatement {
    fn shift(&mut self, shift: &PositionShift) {
        shift.apply_span(&mut self.span);
        for statement in &mut self.statements {
            statement.shift(shift);
        }
    }
}

impl Statement {
    fn shift(&mut self, shift: &PositionShift) {
        match self {
            Statement::Block(block) => block.shift(shift),
            Statement::Variable(variable) => {
                shift.apply_span(&mut variable.span);
                if let Some(type_ref) = &mut variable.type_ref {
                    type_ref.shift(shift);
                }
                if let Some(initializer) = &mut variable.initializer {
                    initializer.shift(shift);
                }
            }
            Statement::Return(value, span) | Statement::Throw(value, span) => {
                shift.apply_span(span);
                if let Some(value) = value {
                    value.shift(shift);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                shift.apply_span(span);
                condition.shift(shift);
                then_branch.shift(shift);
                if let Some(else_branch) = else_branch {
                    else_branch.shift(shift);
                }
            }
            Statement::While {
                condition,
                body,
                span,
            } => {
                shift.apply_span(span);
                condition.shift(shift);
                body.shift(shift);
            }
            Statement::For {
                initializer,
                condition,
                increment,
                body,
                span,
            } => {
                shift.apply_span(span);
                if let Some(initializer) = initializer {
                    initializer.shift(shift);
                }
                if let Some(condition) = condition {
                    condition.shift(shift);
                }
                if let Some(increment) = increment {
                    increment.shift(shift);
                }
                body.shift(shift);
            }
            Statement::ForEach {
                variable_type,
                variable_name: _,
                iterable,
                body,
                span,
            } => {
                shift.apply_span(span);
                variable_type.shift(shift);
                iterable.shift(shift);
                body.shift(shift);
            }
            Statement::Switch { value, cases, span } => {
                shift.apply_span(span);
                value.shift(shift);
                for case in cases {
                    shift.apply_span(&mut case.span);
                    if let Some(value) = &mut case.value {
                        value.shift(shift);
                    }
                    for statement in &mut case.statements {
                        statement.shift(shift);
                    }
                }
            }
            Statement::Break(span) | Statement::Continue(span) | Statement::Empty(span) => {
                shift.apply_span(span);
            }
            Statement::Try {
                body,
                catches,
                finally_block,
                span,
            } => {
                shift.apply_span(span);
                body.shift(shift);
                for catch in catches {
                    shift.apply_span(&mut catch.span);
                    if let Some(exception_type) = &mut catch.exception_type {
                        exception_type.shift(shift);
                    }
                    catch.body.shift(shift);
                }
                if let Some(finally_block) = finally_block {
                    finally_block.shift(shift);
                }
            }
            Statement::Match { value, arms, span } => {
                shift.apply_span(span);
                value.shift(shift);
                for arm in arms {
                    shift.apply_span(&mut arm.span);
                    arm.body.shift(shift);
                }
            }
            Statement::Expression(expression, span) => {
                shift.apply_span(span);
                expression.shift(shift);
            }
        }
    }
}

impl Expression {
    fn shift(&mut self, shift: &PositionShift) {
        match self {
            Expression::Identifier(_, span)
            | Expression::Integer(_, span)
            | Expression::Float(_, span)
            | Expression::String(_, span)
            | Expression::Character(_, span)
            | Expression::Boolean(_, span)
            | Expression::Null(span) => shift.apply_span(span),
            Expression::Group(inner, span) => {
                shift.apply_span(span);
                inner.shift(shift);
            }
            Expression::Unary { operand, span, .. } => {
                shift.apply_span(span);
                operand.shift(shift);
            }
            Expression::Binary {
                left, right, span, ..
            } => {
                shift.apply_span(span);
                left.shift(shift);
                right.shift(shift);
            }
            Expression::Conditional {
                condition,
                when_true,
                when_false,
                span,
            } => {
                shift.apply_span(span);
                condition.shift(shift);
                when_true.shift(shift);
                when_false.shift(shift);
            }
            Expression::Assignment {
                target,
                value,
                span,
                ..
            } => {
                shift.apply_span(span);
                target.shift(shift);
                value.shift(shift);
            }
            Expression::Call {
                callee,
                generic_arguments,
                arguments,
                span,
            } => {
                shift.apply_span(span);
                callee.shift(shift);
                for argument in generic_arguments {
                    argument.shift(shift);
                }
                for argument in arguments {
                    argument.shift(shift);
                }
            }
            Expression::ObjectCreation {
                type_ref,
                arguments,
                span,
            } => {
                shift.apply_span(span);
                type_ref.shift(shift);
                for argument in arguments {
                    argument.shift(shift);
                }
            }
            Expression::Member { object, span, .. } => {
                shift.apply_span(span);
                object.shift(shift);
            }
            Expression::Index {
                object,
                index,
                span,
            } => {
                shift.apply_span(span);
                object.shift(shift);
                index.shift(shift);
            }
        }
    }
}

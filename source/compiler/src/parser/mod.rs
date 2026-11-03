//! Recursive-descent parser with Pratt expression parsing.

use crate::ast::*;
use crate::diagnostics::{Diagnostic, SourcePosition, SourceSpan};
use crate::lexer::{Token, TokenKind};

pub mod incremental;

#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

/// Ein Durchlauf der Top-Level-Schleife: Tokenbereich `start..end`, die dabei
/// entstandene Deklaration (Index in `Program::declarations`) und Diagnosen.
#[derive(Debug, Clone, PartialEq)]
pub struct TopLevelItem {
    pub start: usize,
    pub end: usize,
    pub declaration: Option<usize>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse-Ergebnis samt Zuordnung der Tokenbereiche zu Deklarationen.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFile {
    pub program: Program,
    /// Alle Diagnosen in Parser-Reihenfolge (Kopfbereich, dann je Element).
    pub diagnostics: Vec<Diagnostic>,
    pub header_diagnostics: Vec<Diagnostic>,
    pub items: Vec<TopLevelItem>,
}

impl ParsedFile {
    fn assemble(
        program: Program,
        header_diagnostics: Vec<Diagnostic>,
        items: Vec<TopLevelItem>,
    ) -> Self {
        let diagnostics = header_diagnostics
            .iter()
            .chain(items.iter().flat_map(|item| &item.diagnostics))
            .cloned()
            .collect();
        Self {
            program,
            diagnostics,
            header_diagnostics,
            items,
        }
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn parse(self) -> ParseOutput {
        let parsed = self.parse_traced();
        ParseOutput {
            program: parsed.program,
            diagnostics: parsed.diagnostics,
        }
    }

    /// Wie [`Parser::parse`], merkt sich aber die Tokenbereiche der
    /// Top-Level-Elemente für [`incremental::reparse`].
    pub fn parse_traced(mut self) -> ParsedFile {
        let mut program = Program::empty();
        self.parse_header(&mut program);
        let header_diagnostics = std::mem::take(&mut self.diagnostics);
        let mut items = Vec::new();
        while !self.is_at_end() {
            items.push(self.parse_top_level_item(&mut program));
        }
        ParsedFile::assemble(program, header_diagnostics, items)
    }

    /// `using`- und `namespace`-Direktiven dürfen in beliebiger Reihenfolge
    /// stehen; Deklarationen beginnen, sobald keine mehr folgt.
    fn parse_header(&mut self, program: &mut Program) {
        loop {
            if self.matches_identifier("using") {
                if let Some(using) = self.parse_using() {
                    program.usings.push(using);
                }
            } else if self.matches_identifier("namespace") && program.namespace.is_none() {
                self.advance();
                program.namespace = Some(self.parse_qualified_name());
                self.consume_symbol(";", "Nach dem Namespace wird ';' erwartet.");
            } else {
                break;
            }
        }
    }

    fn parse_top_level_item(&mut self, program: &mut Program) -> TopLevelItem {
        let start = self.current;
        let diagnostics_before = self.diagnostics.len();
        let declaration = if let Some(declaration) = self.parse_declaration() {
            program.declarations.push(declaration);
            Some(program.declarations.len() - 1)
        } else {
            self.synchronize();
            None
        };
        if self.current == start && !self.is_at_end() {
            self.advance();
        }
        TopLevelItem {
            start,
            end: self.current,
            declaration,
            diagnostics: self.diagnostics.split_off(diagnostics_before),
        }
    }

    fn parse_using(&mut self) -> Option<UsingDirective> {
        let start = self.advance().span;
        let path = self.parse_qualified_name();
        let end = self
            .consume_symbol(";", "Nach einer using-Direktive wird ';' erwartet.")
            .map(|t| t.span)
            .unwrap_or(start);
        if path.is_empty() {
            return None;
        }
        Some(UsingDirective {
            path,
            span: join_span(start, end),
        })
    }

    fn parse_declaration(&mut self) -> Option<Declaration> {
        if self.matches_identifier("let") {
            let token = self.advance();
            self.error_at(token.span, "'let' ist reserviert, aber in NORTEX nicht unterstützt. Verwende eine Typangabe für Deklarationen.");
            return None;
        }
        let modifiers = self.parse_modifiers();
        if self.matches_identifier("class")
            || self.matches_identifier("struct")
            || self.matches_identifier("record")
            || self.matches_identifier("interface")
            || self.matches_identifier("trait")
            || self.matches_identifier("enum")
        {
            return self
                .parse_type_declaration(modifiers)
                .map(Declaration::Class);
        }
        // C#-like declaration form: `public int calculate(int value) { ... }`.
        let start = self.peek().span;
        let return_type = self.parse_type_reference()?;
        let name =
            self.consume_identifier("Nach dem Rückgabetyp wird ein Funktionsname erwartet.")?;
        let generic_parameters = self.parse_generic_parameter_names();
        if self.matches_symbol("=") || self.matches_symbol(";") {
            let initializer = if self.matches_symbol("=") {
                self.advance();
                self.parse_expression(1)
            } else {
                None
            };
            let end = self
                .consume_symbol(";", "Nach einer globalen Variable wird ';' erwartet.")
                .map(|token| token.span)
                .unwrap_or(name.span);
            return Some(Declaration::GlobalVariable(FieldDeclaration {
                name: name.lexeme,
                type_ref: return_type,
                modifiers,
                initializer,
                span: join_span(start, end),
            }));
        }
        if !self.matches_symbol("(") {
            self.diagnostics.push(Diagnostic::error_with_code(
                "NX0102",
                "Ungültige Top-Level-Deklaration. Erwartet wird eine Funktion mit '(' oder eine globale Variable mit '=' bzw. ';'.",
                self.peek().span,
            ));
            self.synchronize();
            return None;
        }
        let parameters = self.parse_parameters()?;
        let body = if self.check_symbol("{") {
            Some(self.parse_block()?)
        } else {
            self.consume_symbol(";", "Funktionsdeklaration benötigt einen Block oder ';'.");
            None
        };
        let end = body
            .as_ref()
            .map(|b| b.span)
            .unwrap_or(self.previous().span);
        Some(Declaration::Function(FunctionDeclaration {
            name: name.lexeme,
            generic_parameters,
            return_type,
            parameters,
            modifiers,
            body,
            span: join_span(start, end),
        }))
    }

    fn parse_type_declaration(&mut self, modifiers: Vec<String>) -> Option<ClassDeclaration> {
        let keyword = self.advance();
        let kind = match keyword.lexeme.as_str() {
            "class" => TypeDeclarationKind::Class,
            "struct" => TypeDeclarationKind::Struct,
            "record" => TypeDeclarationKind::Record,
            "interface" => TypeDeclarationKind::Interface,
            "trait" => TypeDeclarationKind::Trait,
            _ => TypeDeclarationKind::Enum,
        };
        let name_token =
            self.consume_identifier("Nach dem Typ-Schlüsselwort wird ein Name erwartet.")?;
        let generic_parameters = self.parse_generic_parameter_names();
        let mut base_types = Vec::new();
        if self.matches_symbol(":") {
            self.advance();
            while let Some(base) = self.parse_type_reference() {
                base_types.push(base);
                if !self.matches_symbol(",") {
                    break;
                }
                self.advance();
            }
        }
        self.consume_symbol("{", "Vor dem Typkörper wird '{' erwartet.");
        let mut members = Vec::new();
        while !self.check_symbol("}") && !self.is_at_end() {
            let before = self.current;
            let member = if kind == TypeDeclarationKind::Enum {
                self.parse_enum_variant()
            } else {
                self.parse_member(&name_token.lexeme)
            };
            if let Some(member) = member {
                members.push(member);
            } else if self.current == before && kind == TypeDeclarationKind::Enum {
                self.synchronize_enum_variant();
            } else if self.current == before {
                self.synchronize_member();
            }
            if self.current == before && !self.is_at_end() {
                self.advance();
            }
        }
        let end = self
            .consume_symbol("}", "Typkörper ist nicht abgeschlossen.")
            .map(|t| t.span)
            .unwrap_or(name_token.span);
        Some(ClassDeclaration {
            kind,
            name: name_token.lexeme,
            generic_parameters,
            modifiers,
            base_types,
            members,
            span: join_span(keyword.span, end),
        })
    }

    fn parse_enum_variant(&mut self) -> Option<MemberDeclaration> {
        if self.matches_symbol(",") || self.matches_symbol(";") {
            self.advance();
            return None;
        }
        let name = self.consume_identifier("Enum-Variante erwartet.")?;
        let mut payload_types = Vec::new();
        if self.matches_symbol("(") {
            self.advance();
            if !self.check_symbol(")") {
                loop {
                    payload_types.push(self.parse_type_reference()?);
                    if !self.matches_symbol(",") {
                        break;
                    }
                    self.advance();
                }
            }
            self.consume_symbol(")", "Nach den Enum-Variantentypen wird ')' erwartet.");
        }
        let end = self.previous().span;
        let variant = EnumVariant {
            name: name.lexeme,
            payload_types,
            span: join_span(name.span, end),
        };
        if self.matches_symbol(",") || self.matches_symbol(";") {
            self.advance();
        }
        Some(MemberDeclaration::Variant(variant))
    }

    fn synchronize_enum_variant(&mut self) {
        while !self.is_at_end() && !self.check_symbol("}") {
            if self.matches_symbol(",") || self.matches_symbol(";") {
                self.advance();
                return;
            }
            if self.peek().span.start.line > self.previous().span.end.line {
                return;
            }
            self.advance();
        }
    }

    fn parse_member(&mut self, owner_name: &str) -> Option<MemberDeclaration> {
        let modifiers = self.parse_modifiers();
        if self.matches_symbol(";") {
            self.advance();
            return None;
        }
        let start = self.peek().span;

        // Constructor syntax is `public Container(T value) { ... }`.
        if self.matches_identifier(owner_name) && self.peek_n(1).lexeme == "(" {
            let name = self.advance();
            let parameters = self.parse_parameters()?;
            let body = if self.check_symbol("{") {
                Some(self.parse_block()?)
            } else {
                self.consume_symbol(";", "Konstruktordeklaration benötigt einen Block oder ';'.");
                None
            };
            let end = body
                .as_ref()
                .map(|block| block.span)
                .unwrap_or(self.previous().span);
            return Some(MemberDeclaration::Method(FunctionDeclaration {
                name: name.lexeme,
                generic_parameters: Vec::new(),
                return_type: TypeReference::simple("void", name.span),
                parameters,
                modifiers,
                body,
                span: join_span(start, end),
            }));
        }

        let type_ref = self.parse_type_reference()?;

        if self.matches_identifier("operator") {
            self.advance();
            let operator_token = self.advance();
            let operator = match operator_token.kind {
                TokenKind::Symbol(ref symbol) => symbol.clone(),
                _ => {
                    self.error_at(
                        operator_token.span,
                        "Nach 'operator' wird ein Operatorzeichen erwartet.",
                    );
                    String::new()
                }
            };
            let parameters = self.parse_parameters()?;
            let body = if self.check_symbol("{") {
                Some(self.parse_block()?)
            } else {
                self.consume_symbol(";", "Operatorüberladung benötigt einen Block oder ';'.");
                None
            };
            let end = body
                .as_ref()
                .map(|b| b.span)
                .unwrap_or(self.previous().span);
            return Some(MemberDeclaration::Operator(OperatorDeclaration {
                operator,
                return_type: type_ref,
                parameters,
                modifiers,
                body,
                span: join_span(start, end),
            }));
        }

        let name = self.consume_identifier("Nach dem Typ wird ein Membername erwartet.")?;
        let generic_parameters = self.parse_generic_parameter_names();
        if self.check_symbol("(") {
            let parameters = self.parse_parameters()?;
            let body = if self.check_symbol("{") {
                Some(self.parse_block()?)
            } else {
                self.consume_symbol(";", "Methodendeklaration benötigt einen Block oder ';'.");
                None
            };
            let end = body
                .as_ref()
                .map(|b| b.span)
                .unwrap_or(self.previous().span);
            Some(MemberDeclaration::Method(FunctionDeclaration {
                name: name.lexeme,
                generic_parameters,
                return_type: type_ref,
                parameters,
                modifiers,
                body,
                span: join_span(start, end),
            }))
        } else {
            let initializer = if self.matches_symbol("=") {
                self.advance();
                self.parse_expression(1)
            } else {
                None
            };
            let end = self
                .consume_symbol(";", "Nach einem Feld wird ';' erwartet.")
                .map(|t| t.span)
                .unwrap_or(name.span);
            let _ = owner_name;
            Some(MemberDeclaration::Field(FieldDeclaration {
                name: name.lexeme,
                type_ref,
                modifiers,
                initializer,
                span: join_span(start, end),
            }))
        }
    }

    fn parse_parameters(&mut self) -> Option<Vec<Parameter>> {
        self.consume_symbol("(", "Vor den Parametern wird '(' erwartet.")?;
        let mut parameters = Vec::new();
        if !self.check_symbol(")") {
            loop {
                let start = self.peek().span;
                let type_ref = self.parse_type_reference()?;
                let name =
                    self.consume_identifier("Nach dem Parametertyp wird ein Name erwartet.")?;
                parameters.push(Parameter {
                    name: name.lexeme,
                    type_ref,
                    span: join_span(start, name.span),
                });
                if !self.matches_symbol(",") {
                    break;
                }
                self.advance();
            }
        }
        self.consume_symbol(")", "Nach den Parametern wird ')' erwartet.")?;
        Some(parameters)
    }

    fn parse_generic_parameter_names(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        if !self.matches_symbol("<") {
            return names;
        }
        self.advance();
        while let Some(name) = self.consume_identifier("Generischer Parametername erwartet.") {
            names.push(name.lexeme);
            if !self.matches_symbol(",") {
                break;
            }
            self.advance();
        }
        self.consume_symbol(
            ">",
            "Generische Parameter benötigen ein abschließendes '>'.",
        );
        names
    }

    fn parse_type_reference(&mut self) -> Option<TypeReference> {
        let start = self.peek().span;
        let reference_kind = if self.matches_symbol("&") {
            self.advance();
            if self.matches_identifier("mut") {
                self.advance();
                ReferenceKind::MutableBorrow
            } else {
                ReferenceKind::SharedBorrow
            }
        } else if self.matches_symbol("*") {
            self.advance();
            if self.matches_identifier("mut") {
                self.advance();
                ReferenceKind::RawMutablePointer
            } else if self.matches_identifier("const") {
                self.advance();
                ReferenceKind::RawConstPointer
            } else {
                self.error_at(
                    self.peek().span,
                    "Ein Raw-Pointer benötigt 'const' oder 'mut'.",
                );
                ReferenceKind::RawConstPointer
            }
        } else {
            ReferenceKind::Value
        };

        // Never type is written as `!` in return/type position.
        if self.matches_symbol("!") {
            let end = self.advance().span;
            return Some(TypeReference {
                name: "!".to_string(),
                generic_arguments: Vec::new(),
                array_depth: 0,
                fixed_array_length: None,
                reference_kind,
                span: join_span(start, end),
            });
        }

        // Fixed-size array type: [ElementType; N].
        if self.matches_symbol("[") {
            self.advance();
            let mut element = self.parse_type_reference()?;
            self.consume_symbol(
                ";",
                "In einem festen Arraytyp wird ';' zwischen Typ und Länge erwartet.",
            );
            let length_token = self.advance();
            let length =
                if length_token.kind == TokenKind::Number && !length_token.lexeme.contains('.') {
                    length_token.lexeme.replace('_', "").parse::<usize>().ok()
                } else {
                    None
                };
            if length.is_none() {
                self.error_at(length_token.span, "Die Länge eines festen Arrays muss eine nichtnegative ganzzahlige Compile-Time-Konstante sein.");
            }
            let end = self
                .consume_symbol("]", "Ein fester Arraytyp benötigt ein abschließendes ']'.")
                .map(|token| token.span)
                .unwrap_or(length_token.span);
            element.fixed_array_length = length;
            element.reference_kind = reference_kind;
            element.span = join_span(start, end);
            return Some(element);
        }

        let first = self.consume_type_name("Typname erwartet.")?;
        let mut name = first.lexeme;
        while self.matches_symbol(".") || self.matches_symbol("::") {
            self.advance();
            let part =
                self.consume_identifier("Nach dem Punkt wird ein weiterer Typname erwartet.")?;
            name.push('.');
            name.push_str(&part.lexeme);
        }
        let mut generic_arguments = Vec::new();
        if self.matches_symbol("<") {
            self.advance();
            loop {
                generic_arguments.push(self.parse_type_reference()?);
                if !self.matches_symbol(",") {
                    break;
                }
                self.advance();
            }
            self.consume_type_generic_close(
                "Generische Typargumente benötigen ein abschließendes '>'.",
            );
        }
        let mut array_depth = 0;
        while self.matches_symbol("[") {
            self.advance();
            self.consume_symbol("]", "Nach '[' wird ']' erwartet.");
            array_depth += 1;
        }
        let end = self.previous().span;
        Some(TypeReference {
            name,
            generic_arguments,
            array_depth,
            fixed_array_length: None,
            reference_kind,
            span: join_span(start, end),
        })
    }

    fn parse_block(&mut self) -> Option<BlockStatement> {
        let start = self.consume_symbol("{", "Blockbeginn '{' erwartet.")?.span;
        let mut statements = Vec::new();
        while !self.check_symbol("}") && !self.is_at_end() {
            let before = self.current;
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            } else {
                self.synchronize_statement();
            }
            if self.current == before && !self.is_at_end() {
                self.advance();
            }
        }
        let end = self
            .consume_symbol("}", "Block ist nicht abgeschlossen.")?
            .span;
        Some(BlockStatement {
            statements,
            span: join_span(start, end),
        })
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        if self.matches_identifier("let") {
            let token = self.advance();
            self.error_at(token.span, "'let' ist reserviert, aber in NORTEX nicht unterstützt. Verwende eine Typangabe, z. B. 'int value = 1;'.");
            self.synchronize_statement();
            return None;
        }
        if self.check_symbol("{") {
            return self.parse_block().map(Statement::Block);
        }
        if self.matches_symbol(";") {
            return Some(Statement::Empty(self.advance().span));
        }
        if self.matches_identifier("return") {
            let start = self.advance().span;
            let value = if self.check_symbol(";") {
                None
            } else {
                self.parse_expression(1)
            };
            let end = self
                .consume_symbol(";", "Nach return wird ';' erwartet.")
                .map(|t| t.span)
                .unwrap_or(start);
            return Some(Statement::Return(value, join_span(start, end)));
        }
        if self.matches_identifier("throw") {
            let start = self.advance().span;
            let value = if self.check_symbol(";") {
                None
            } else {
                self.parse_expression(1)
            };
            let end = self
                .consume_symbol(";", "Nach throw wird ';' erwartet.")
                .map(|t| t.span)
                .unwrap_or(start);
            return Some(Statement::Throw(value, join_span(start, end)));
        }
        if self.matches_identifier("if") {
            let start = self.advance().span;
            self.consume_symbol("(", "Nach if wird '(' erwartet.");
            let condition = self.parse_expression(1)?;
            self.consume_symbol(")", "Nach der if-Bedingung wird ')' erwartet.");
            let then_branch = Box::new(self.parse_statement()?);
            let else_branch = if self.matches_identifier("else") {
                self.advance();
                Some(Box::new(self.parse_statement()?))
            } else {
                None
            };
            let end = else_branch
                .as_ref()
                .map(|b| statement_span(b))
                .unwrap_or_else(|| statement_span(&then_branch));
            return Some(Statement::If {
                condition,
                then_branch,
                else_branch,
                span: join_span(start, end),
            });
        }
        if self.matches_identifier("while") {
            let start = self.advance().span;
            self.consume_symbol("(", "Nach while wird '(' erwartet.");
            let condition = self.parse_expression(1)?;
            self.consume_symbol(")", "Nach der while-Bedingung wird ')' erwartet.");
            let body = Box::new(self.parse_statement()?);
            let span = join_span(start, statement_span(&body));
            return Some(Statement::While {
                condition,
                body,
                span,
            });
        }
        if self.matches_identifier("for") {
            return self.parse_for_statement();
        }
        if self.matches_identifier("switch") {
            return self.parse_switch_statement();
        }
        if self.matches_identifier("break") {
            let start = self.advance().span;
            let end = self
                .consume_symbol(";", "Nach break wird ';' erwartet.")
                .map(|t| t.span)
                .unwrap_or(start);
            return Some(Statement::Break(join_span(start, end)));
        }
        if self.matches_identifier("continue") {
            let start = self.advance().span;
            let end = self
                .consume_symbol(";", "Nach continue wird ';' erwartet.")
                .map(|t| t.span)
                .unwrap_or(start);
            return Some(Statement::Continue(join_span(start, end)));
        }
        if self.matches_identifier("try") {
            return self.parse_try_statement();
        }
        if self.matches_identifier("match") {
            return self.parse_match_statement();
        }
        if self.matches_identifier("const")
            || self.matches_identifier("mut")
            || self.matches_identifier("var")
            || self.looks_like_variable_declaration()
        {
            return self.parse_variable_declaration();
        }
        let start = self.peek().span;
        let expression = self.parse_expression(1)?;
        let end = self
            .consume_symbol(";", "Nach einem Ausdruck wird ';' erwartet.")
            .map(|t| t.span)
            .unwrap_or(expression.span());
        Some(Statement::Expression(expression, join_span(start, end)))
    }

    fn parse_try_statement(&mut self) -> Option<Statement> {
        let start = self.advance().span;
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        while self.matches_identifier("catch") {
            let catch_start = self.advance().span;
            let (exception_type, variable_name) = if self.matches_symbol("(") {
                self.advance();
                let type_ref = self.parse_type_reference();
                let name = if self.check_kind(&TokenKind::Identifier) {
                    Some(self.advance().lexeme)
                } else {
                    None
                };
                self.consume_symbol(")", "Nach catch-Parameter wird ')' erwartet.");
                (type_ref, name)
            } else {
                (None, None)
            };
            let catch_body = self.parse_block()?;
            catches.push(CatchClause {
                exception_type,
                variable_name,
                span: join_span(catch_start, catch_body.span),
                body: catch_body,
            });
        }
        let finally_block = if self.matches_identifier("finally") {
            self.advance();
            self.parse_block()
        } else {
            None
        };
        if catches.is_empty() && finally_block.is_none() {
            self.error_at(start, "Auf try muss mindestens catch oder finally folgen.");
        }
        let end = finally_block
            .as_ref()
            .map(|b| b.span)
            .or_else(|| catches.last().map(|c| c.span))
            .unwrap_or(body.span);
        Some(Statement::Try {
            body,
            catches,
            finally_block,
            span: join_span(start, end),
        })
    }

    fn parse_for_statement(&mut self) -> Option<Statement> {
        let start = self.advance().span;
        self.consume_symbol("(", "Nach for wird '(' erwartet.");

        // `for (T value in sequence)` is the NORTEX foreach form. It is
        // detected before parsing the classic three-part for header.
        if self.looks_like_foreach_header() {
            let variable_type = self.parse_type_reference()?;
            let variable =
                self.consume_identifier("Nach dem foreach-Typ wird ein Variablenname erwartet.")?;
            self.consume_keyword_or_identifier(
                "in",
                "Zwischen foreach-Variable und Sequenz wird 'in' erwartet.",
            )?;
            let iterable = self.parse_expression(1)?;
            self.consume_symbol(")", "Nach der foreach-Sequenz wird ')' erwartet.");
            let body = Box::new(self.parse_statement()?);
            let span = join_span(start, statement_span(&body));
            return Some(Statement::ForEach {
                variable_type,
                variable_name: variable.lexeme,
                iterable,
                body,
                span,
            });
        }

        let initializer = if self.matches_symbol(";") {
            self.advance();
            None
        } else if self.matches_identifier("var")
            || self.matches_identifier("const")
            || self.looks_like_variable_declaration()
        {
            Some(Box::new(self.parse_variable_declaration()?))
        } else {
            let expression_start = self.peek().span;
            let expression = self.parse_expression(1)?;
            let end = self
                .consume_symbol(";", "Nach der for-Initialisierung wird ';' erwartet.")
                .map(|t| t.span)
                .unwrap_or(expression.span());
            Some(Box::new(Statement::Expression(
                expression,
                join_span(expression_start, end),
            )))
        };

        let condition = if self.check_symbol(";") {
            None
        } else {
            self.parse_expression(1)
        };
        self.consume_symbol(";", "Nach der for-Bedingung wird ';' erwartet.");
        let increment = if self.check_symbol(")") {
            None
        } else {
            self.parse_expression(1)
        };
        self.consume_symbol(")", "Nach dem for-Header wird ')' erwartet.");
        let body = Box::new(self.parse_statement()?);
        Some(Statement::For {
            initializer,
            condition,
            increment,
            span: join_span(start, statement_span(&body)),
            body,
        })
    }

    fn looks_like_foreach_header(&self) -> bool {
        self.is_type_name_token(self.peek())
            && self.peek_n(1).kind == TokenKind::Identifier
            && self.peek_n(2).lexeme == "in"
    }

    fn consume_keyword_or_identifier(&mut self, value: &str, message: &str) -> Option<Token> {
        if self.matches_identifier(value) {
            Some(self.advance())
        } else {
            self.error_at(self.peek().span, message);
            None
        }
    }

    fn parse_switch_statement(&mut self) -> Option<Statement> {
        let start = self.advance().span;
        self.consume_symbol("(", "Nach switch wird '(' erwartet.");
        let value = self.parse_expression(1)?;
        self.consume_symbol(")", "Nach dem switch-Ausdruck wird ')' erwartet.");
        self.consume_symbol("{", "Vor den switch-Fällen wird '{' erwartet.");
        let mut cases = Vec::new();
        while !self.check_symbol("}") && !self.is_at_end() {
            let case_start = self.peek().span;
            let case_value = if self.matches_identifier("case") {
                self.advance();
                Some(self.parse_expression(1)?)
            } else if self.matches_identifier("default") {
                self.advance();
                None
            } else {
                self.error_at(
                    self.peek().span,
                    "In einem switch-Block wird 'case' oder 'default' erwartet.",
                );
                self.synchronize_statement();
                continue;
            };
            self.consume_symbol(":", "Nach einem switch-Label wird ':' erwartet.");
            let mut statements = Vec::new();
            while !self.check_symbol("}")
                && !self.matches_identifier("case")
                && !self.matches_identifier("default")
                && !self.is_at_end()
            {
                let before = self.current;
                if let Some(statement) = self.parse_statement() {
                    statements.push(statement);
                } else {
                    self.synchronize_statement();
                }
                if self.current == before && !self.is_at_end() {
                    self.advance();
                }
            }
            let end = statements.last().map(statement_span).unwrap_or(case_start);
            cases.push(SwitchCase {
                value: case_value,
                statements,
                span: join_span(case_start, end),
            });
        }
        let end = self
            .consume_symbol("}", "Switch-Block ist nicht abgeschlossen.")?
            .span;
        Some(Statement::Switch {
            value,
            cases,
            span: join_span(start, end),
        })
    }

    fn parse_match_statement(&mut self) -> Option<Statement> {
        let start = self.advance().span;
        self.consume_symbol("(", "Nach match wird '(' erwartet.");
        let value = self.parse_expression(1)?;
        self.consume_symbol(")", "Nach dem Match-Ausdruck wird ')' erwartet.");
        self.consume_symbol("{", "Vor den Match-Armen wird '{' erwartet.");
        let mut arms = Vec::new();
        while !self.check_symbol("}") && !self.is_at_end() {
            let before = self.current;
            let arm_start = self.peek().span;
            let variant = self.consume_identifier("Match-Variante oder '_' erwartet.")?;
            let variant_name = if self.matches_symbol(".") {
                self.advance();
                let member =
                    self.consume_identifier("Nach dem Enum-Namen wird eine Variante erwartet.")?;
                format!("{}.{}", variant.lexeme, member.lexeme)
            } else {
                variant.lexeme
            };
            let binding = if self.matches_symbol("(") {
                self.advance();
                let binding = self.consume_identifier("Bindungsname im Match-Arm erwartet.")?;
                self.consume_symbol(")", "Nach dem Match-Bindungsnamen wird ')' erwartet.");
                Some(binding.lexeme)
            } else {
                None
            };
            self.consume_symbol("=>", "Nach dem Match-Pattern wird '=>' erwartet.");
            let body = if self.check_symbol("{") {
                self.parse_block()?
            } else {
                let expression = self.parse_expression(1)?;
                let end = self
                    .consume_symbol(
                        ";",
                        "Nach einem Ausdruck in einem Match-Arm wird ';' erwartet.",
                    )?
                    .span;
                let statement = Statement::Expression(expression, join_span(arm_start, end));
                BlockStatement {
                    statements: vec![statement],
                    span: join_span(arm_start, end),
                }
            };
            let span = join_span(arm_start, body.span);
            arms.push(MatchArm {
                variant: variant_name,
                binding,
                body,
                span,
            });
            if self.matches_symbol(",") || self.matches_symbol(";") {
                self.advance();
            }
            if self.current == before {
                self.advance();
            }
        }
        let end = self
            .consume_symbol("}", "Match-Block ist nicht abgeschlossen.")?
            .span;
        Some(Statement::Match {
            value,
            arms,
            span: join_span(start, end),
        })
    }

    fn looks_like_variable_declaration(&self) -> bool {
        if self.matches_identifier("var")
            || self.matches_identifier("const")
            || self.matches_identifier("mut")
        {
            return true;
        }
        if self.matches_symbol("&") || self.matches_symbol("*") {
            let mut type_index = self.current + 1;
            if self.matches_symbol("*") {
                if !self
                    .tokens
                    .get(type_index)
                    .is_some_and(|token| matches!(token.lexeme.as_str(), "const" | "mut"))
                {
                    return false;
                }
                type_index += 1;
            } else if self
                .tokens
                .get(type_index)
                .is_some_and(|token| token.lexeme == "mut")
            {
                type_index += 1;
            }
            return self
                .tokens
                .get(type_index)
                .is_some_and(|token| self.is_type_name_token(token))
                && self
                    .tokens
                    .get(type_index + 1)
                    .is_some_and(|token| token.kind == TokenKind::Identifier);
        }
        if self.matches_symbol("[") {
            let mut index = self.current + 1;
            while index < self.tokens.len()
                && self.tokens[index].lexeme != "]"
                && self.tokens[index].kind != TokenKind::EndOfFile
            {
                index += 1;
            }
            return self
                .tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier);
        }
        if !self.is_type_name_token(self.peek()) {
            return false;
        }
        if self.peek_n(1).kind == TokenKind::Identifier {
            return true;
        }
        if self.peek_n(1).lexeme == "["
            && self.peek_n(2).lexeme == "]"
            && self.peek_n(3).kind == TokenKind::Identifier
        {
            return true;
        }
        if self.peek_n(1).lexeme != "<" {
            return false;
        }

        // Look through `GenericType<A, B> variableName` without consuming tokens.
        let mut index = self.current + 1;
        let mut depth = 0usize;
        while index < self.tokens.len() {
            let lexeme = self.tokens[index].lexeme.as_str();
            if lexeme == "<" {
                depth += 1;
            }
            if lexeme == ">" {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return self
                        .tokens
                        .get(index + 1)
                        .is_some_and(|token| token.kind == TokenKind::Identifier);
                }
            }
            if lexeme == ">>" {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return self
                        .tokens
                        .get(index + 1)
                        .is_some_and(|token| token.kind == TokenKind::Identifier);
                }
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return self
                        .tokens
                        .get(index + 1)
                        .is_some_and(|token| token.kind == TokenKind::Identifier);
                }
            }
            if self.tokens[index].kind == TokenKind::EndOfFile {
                break;
            }
            index += 1;
        }
        false
    }

    fn parse_variable_declaration(&mut self) -> Option<Statement> {
        let start = self.peek().span;
        let mutability = if self.matches_identifier("const") {
            self.advance();
            VariableMutability::CompileTime
        } else if self.matches_identifier("mut") {
            self.advance();
            VariableMutability::Mutable
        } else {
            VariableMutability::Immutable
        };
        let (type_ref, name) = if self.matches_identifier("var") {
            self.advance();
            (
                None,
                self.consume_identifier("Nach var wird ein Variablenname erwartet.")?,
            )
        } else {
            let ty = self.parse_type_reference()?;
            let name = self.consume_identifier("Nach dem Typ wird ein Variablenname erwartet.")?;
            (Some(ty), name)
        };
        let initializer = if self.matches_symbol("=") {
            self.advance();
            self.parse_expression(1)
        } else {
            None
        };
        let end = self
            .consume_symbol(";", "Nach einer Variablendeklaration wird ';' erwartet.")
            .map(|t| t.span)
            .unwrap_or(name.span);
        Some(Statement::Variable(VariableDeclaration {
            name: name.lexeme,
            type_ref,
            initializer,
            mutability,
            span: join_span(start, end),
        }))
    }

    fn parse_expression(&mut self, min_precedence: u8) -> Option<Expression> {
        let mut left = self.parse_unary()?;
        loop {
            // Postfix expressions bind more tightly than all infix operators.
            if self.looks_like_generic_invocation_at_current() {
                self.consume_symbol("<", "Generische Aufrufargumente benötigen '<'.")?;
                let mut generic_arguments = Vec::new();
                loop {
                    generic_arguments.push(self.parse_type_reference()?);
                    if !self.matches_symbol(",") {
                        break;
                    }
                    self.advance();
                }
                self.consume_symbol(">", "Generische Aufrufargumente benötigen '>'.")?;
                self.consume_symbol("(", "Nach generischen Aufrufargumenten wird '(' erwartet.")?;
                let mut arguments = Vec::new();
                if !self.check_symbol(")") {
                    loop {
                        arguments.push(self.parse_expression(1)?);
                        if !self.matches_symbol(",") {
                            break;
                        }
                        self.advance();
                    }
                }
                let end = self
                    .consume_symbol(")", "Nach generischen Aufrufargumenten wird ')' erwartet.")?
                    .span;
                let span = join_span(left.span(), end);
                left = Expression::Call {
                    callee: Box::new(left),
                    generic_arguments,
                    arguments,
                    span,
                };
                continue;
            }
            if self.check_symbol("(") {
                self.advance();
                let mut arguments = Vec::new();
                if !self.check_symbol(")") {
                    loop {
                        arguments.push(self.parse_expression(1)?);
                        if !self.matches_symbol(",") {
                            break;
                        }
                        self.advance();
                    }
                }
                let end = self
                    .consume_symbol(")", "Nach Funktionsargumenten wird ')' erwartet.")?
                    .span;
                let span = join_span(left.span(), end);
                left = Expression::Call {
                    callee: Box::new(left),
                    generic_arguments: Vec::new(),
                    arguments,
                    span,
                };
                continue;
            }
            if self.matches_symbol(".") {
                self.advance();
                let name = if self.matches_identifier("new") {
                    self.advance()
                } else {
                    self.consume_identifier("Nach '.' wird ein Membername erwartet.")?
                };
                let span = join_span(left.span(), name.span);
                left = Expression::Member {
                    object: Box::new(left),
                    name: name.lexeme,
                    span,
                };
                continue;
            }
            if self.matches_symbol("[") {
                self.advance();
                let index = self.parse_expression(1)?;
                let end = self
                    .consume_symbol("]", "Nach einem Index wird ']' erwartet.")?
                    .span;
                let span = join_span(left.span(), end);
                left = Expression::Index {
                    object: Box::new(left),
                    index: Box::new(index),
                    span,
                };
                continue;
            }
            if self.matches_symbol("?") && min_precedence <= 2 {
                let question = self.advance();
                if self.check_symbol(";") || self.check_symbol("}") || self.is_at_end() {
                    self.error_at(question.span, "Ein '?' beginnt in NORTEX den Ternäroperator und benötigt ':'. Es gibt keinen '?'-Fehlerweitergabeoperator.");
                    continue;
                }
                let when_true = self.parse_expression(1)?;
                if !self.matches_symbol(":") {
                    self.error_at(
                        question.span,
                        "Nach dem wahren Ausdruck des Ternäroperators wird ':' erwartet.",
                    );
                    return Some(left);
                }
                self.advance();
                let when_false = self.parse_expression(2)?;
                let span = join_span(left.span(), when_false.span());
                left = Expression::Conditional {
                    condition: Box::new(left),
                    when_true: Box::new(when_true),
                    when_false: Box::new(when_false),
                    span,
                };
                continue;
            }
            if self.matches_symbol("++") || self.matches_symbol("--") {
                let operator = self.advance();
                let span = join_span(left.span(), operator.span);
                left = Expression::Unary {
                    operator: format!("post{}", operator.lexeme),
                    operand: Box::new(left),
                    span,
                };
                continue;
            }

            let operator = self.peek().lexeme.clone();
            let Some((precedence, right_associative, is_assignment)) = infix_precedence(&operator)
            else {
                break;
            };
            if precedence < min_precedence {
                break;
            }
            self.advance();
            let next_min = if right_associative {
                precedence
            } else {
                precedence + 1
            };
            let right = self.parse_expression(next_min)?;
            let span = join_span(left.span(), right.span());
            left = if is_assignment {
                Expression::Assignment {
                    target: Box::new(left),
                    operator,
                    value: Box::new(right),
                    span,
                }
            } else {
                Expression::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                    span,
                }
            };
        }
        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Expression> {
        if self.matches_identifier("new") {
            let start = self.advance().span;
            let type_ref = self.parse_type_reference()?;
            let mut arguments = Vec::new();
            if self.matches_symbol("(") {
                self.advance();
                if !self.check_symbol(")") {
                    loop {
                        arguments.push(self.parse_expression(1)?);
                        if !self.matches_symbol(",") {
                            break;
                        }
                        self.advance();
                    }
                }
                self.consume_symbol(")", "Nach Konstruktorargumenten wird ')' erwartet.");
            }
            let end = self.previous().span;
            return Some(Expression::ObjectCreation {
                type_ref,
                arguments,
                span: join_span(start, end),
            });
        }
        if self.matches_identifier("await") {
            let operator = self.advance();
            let operand = self.parse_expression(12)?;
            let span = join_span(operator.span, operand.span());
            return Some(Expression::Unary {
                operator: operator.lexeme,
                operand: Box::new(operand),
                span,
            });
        }
        if self.matches_symbol("!")
            || self.matches_symbol("-")
            || self.matches_symbol("+")
            || self.matches_symbol("~")
            || self.matches_symbol("&")
            || self.matches_symbol("*")
            || self.matches_symbol("++")
            || self.matches_symbol("--")
        {
            let operator = self.advance();
            let operator_name = if operator.lexeme == "&" && self.matches_identifier("mut") {
                self.advance();
                "&mut".to_string()
            } else {
                operator.lexeme.clone()
            };
            let operand = self.parse_expression(12)?;
            let span = join_span(operator.span, operand.span());
            return Some(Expression::Unary {
                operator: operator_name,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Expression> {
        let token = self.advance();
        match token.kind {
            TokenKind::Identifier => {
                if self.looks_like_generic_invocation_from_previous() {
                    self.current = self.current.saturating_sub(1);
                    let type_ref = self.parse_type_reference()?;
                    if self.check_symbol("(") {
                        self.advance();
                        let mut arguments = Vec::new();
                        if !self.check_symbol(")") {
                            loop {
                                arguments.push(self.parse_expression(1)?);
                                if !self.matches_symbol(",") {
                                    break;
                                }
                                self.advance();
                            }
                        }
                        let end = self
                            .consume_symbol(
                                ")",
                                "Nach generischen Aufrufargumenten wird ')' erwartet.",
                            )?
                            .span;
                        Some(Expression::Call {
                            callee: Box::new(Expression::Identifier(type_ref.name, type_ref.span)),
                            generic_arguments: type_ref.generic_arguments,
                            arguments,
                            span: join_span(type_ref.span, end),
                        })
                    } else {
                        Some(Expression::Identifier(token.lexeme, token.span))
                    }
                } else {
                    Some(Expression::Identifier(token.lexeme, token.span))
                }
            }
            TokenKind::Keyword => match token.lexeme.as_str() {
                "true" => Some(Expression::Boolean(true, token.span)),
                "false" => Some(Expression::Boolean(false, token.span)),
                "null" => Some(Expression::Null(token.span)),
                "this" => Some(Expression::Identifier(token.lexeme, token.span)),
                _ => {
                    self.error_at(
                        token.span,
                        "Schlüsselwort ist an dieser Stelle kein Ausdruck.",
                    );
                    None
                }
            },
            TokenKind::Number => {
                let lower = token.lexeme.to_ascii_lowercase();
                let radix_prefixed = lower.starts_with("0x") || lower.starts_with("0b");
                if !radix_prefixed
                    && (lower.contains('.')
                        || lower.contains('e')
                        || lower.ends_with('f')
                        || lower.ends_with('d')
                        || lower.ends_with("dec"))
                {
                    Some(Expression::Float(token.lexeme, token.span))
                } else {
                    Some(Expression::Integer(token.lexeme, token.span))
                }
            }
            TokenKind::StringLiteral => Some(Expression::String(
                unescape_string(&token.lexeme),
                token.span,
            )),
            TokenKind::CharacterLiteral => Some(Expression::Character(
                unescape_character(&token.lexeme),
                token.span,
            )),
            TokenKind::Symbol(symbol) if symbol == "(" => {
                let expression = self.parse_expression(1)?;
                let end = self
                    .consume_symbol(")", "Nach einem gruppierten Ausdruck wird ')' erwartet.")?
                    .span;
                Some(Expression::Group(
                    Box::new(expression),
                    join_span(token.span, end),
                ))
            }
            _ => {
                self.error_at(token.span, "Ausdruck erwartet.");
                None
            }
        }
    }

    fn looks_like_generic_invocation_from_previous(&self) -> bool {
        self.looks_like_generic_invocation_at_current()
    }

    fn looks_like_generic_invocation_at_current(&self) -> bool {
        // Avoid treating comparisons as generic arguments unless `>` is followed by `(`.
        if !self.matches_symbol("<") {
            return false;
        }
        let mut index = self.current;
        let mut depth = 0usize;
        while index < self.tokens.len() {
            match self.tokens[index].lexeme.as_str() {
                "<" => depth += 1,
                ">" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return self.tokens.get(index + 1).is_some_and(|t| t.lexeme == "(");
                    }
                }
                ">>" => {
                    depth = depth.saturating_sub(2);
                    if depth == 0 {
                        return self.tokens.get(index + 1).is_some_and(|t| t.lexeme == "(");
                    }
                }
                _ => {}
            }
            if self.tokens[index].kind == TokenKind::EndOfFile {
                break;
            }
            index += 1;
        }
        false
    }

    fn parse_modifiers(&mut self) -> Vec<String> {
        const MODIFIERS: &[&str] = &[
            "public",
            "private",
            "protected",
            "internal",
            "static",
            "abstract",
            "sealed",
            "partial",
            "extern",
            "intrinsic",
            "unsafe",
            "const",
            "mut",
            "virtual",
            "override",
            "async",
            "readonly",
        ];
        let mut modifiers = Vec::new();
        while self.matches_identifier_in(MODIFIERS) {
            let modifier = self.advance().lexeme;
            let is_extern = modifier == "extern";
            modifiers.push(modifier);
            // `extern "C"` wird als zusätzlicher Pseudo-Modifikator `abi:C` abgelegt.
            if is_extern && self.check_kind(&TokenKind::StringLiteral) {
                let abi = unescape_string(&self.advance().lexeme);
                modifiers.push(format!("abi:{abi}"));
            }
        }
        modifiers
    }

    fn parse_qualified_name(&mut self) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some(first) = self.consume_identifier("Bezeichner erwartet.") {
            parts.push(first.lexeme);
        }
        while self.matches_symbol(".") {
            self.advance();
            if let Some(part) = self.consume_identifier("Nach '.' wird ein Bezeichner erwartet.") {
                parts.push(part.lexeme);
            } else {
                break;
            }
        }
        parts
    }

    fn consume_identifier(&mut self, message: &str) -> Option<Token> {
        if self.check_kind(&TokenKind::Identifier) {
            Some(self.advance())
        } else {
            self.error_at(self.peek().span, message);
            None
        }
    }

    fn consume_type_name(&mut self, message: &str) -> Option<Token> {
        if self.is_type_name_token(self.peek()) {
            Some(self.advance())
        } else {
            self.error_at(self.peek().span, message);
            None
        }
    }

    fn is_type_name_token(&self, token: &Token) -> bool {
        token.kind == TokenKind::Identifier
            || (token.kind == TokenKind::Keyword
                && matches!(
                    token.lexeme.as_str(),
                    "sbyte"
                        | "short"
                        | "int"
                        | "long"
                        | "i128"
                        | "byte"
                        | "ushort"
                        | "uint"
                        | "ulong"
                        | "u128"
                        | "nint"
                        | "nuint"
                        | "isize"
                        | "usize"
                        | "float"
                        | "double"
                        | "decimal"
                        | "bool"
                        | "char"
                        | "string"
                        | "void"
                ))
    }

    /// Consumes one generic closing angle. The lexer keeps `>>` as a shift
    /// operator, so nested generic types split that token into two `>` tokens
    /// only when the parser is in a type-argument context.
    fn consume_type_generic_close(&mut self, message: &str) -> Option<Token> {
        if self.check_symbol(">") {
            return Some(self.advance());
        }
        if self.check_symbol(">>") {
            let original = self.peek().clone();
            let middle = SourcePosition {
                offset: original.span.start.offset + 1,
                line: original.span.start.line,
                column: original.span.start.column + 1,
            };
            let first = Token {
                kind: TokenKind::Symbol(">".to_string()),
                lexeme: ">".to_string(),
                span: SourceSpan {
                    start: original.span.start,
                    end: middle,
                },
            };
            let second = Token {
                kind: TokenKind::Symbol(">".to_string()),
                lexeme: ">".to_string(),
                span: SourceSpan {
                    start: middle,
                    end: original.span.end,
                },
            };
            self.tokens[self.current] = first;
            self.tokens.insert(self.current + 1, second);
            return Some(self.advance());
        }
        self.error_at(self.peek().span, message);
        None
    }

    fn consume_symbol(&mut self, symbol: &str, message: &str) -> Option<Token> {
        if self.check_symbol(symbol) {
            Some(self.advance())
        } else {
            self.error_at(self.peek().span, message);
            None
        }
    }

    fn matches_identifier(&self, name: &str) -> bool {
        matches!(
            &self.peek().kind,
            TokenKind::Identifier | TokenKind::Keyword
        ) && self.peek().lexeme == name
    }

    fn matches_identifier_in(&self, names: &[&str]) -> bool {
        matches!(
            &self.peek().kind,
            TokenKind::Identifier | TokenKind::Keyword
        ) && names.contains(&self.peek().lexeme.as_str())
    }

    fn matches_symbol(&self, symbol: &str) -> bool {
        self.check_symbol(symbol)
    }
    fn check_symbol(&self, symbol: &str) -> bool {
        self.peek().lexeme == symbol
    }
    fn check_kind(&self, kind: &TokenKind) -> bool {
        &self.peek().kind == kind
    }
    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::EndOfFile
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        if !self.is_at_end() {
            self.current += 1;
        }
        token
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current.min(self.tokens.len() - 1)]
    }
    fn peek_n(&self, n: usize) -> &Token {
        &self.tokens[(self.current + n).min(self.tokens.len() - 1)]
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.current.saturating_sub(1)]
    }

    fn error_at(&mut self, span: SourceSpan, message: &str) {
        self.diagnostics.push(Diagnostic::error(message, span));
    }

    fn synchronize(&mut self) {
        if !self.is_at_end() {
            self.advance();
        }
        while !self.is_at_end() {
            if self.previous().lexeme == ";" {
                return;
            }
            if [
                "class",
                "struct",
                "interface",
                "trait",
                "enum",
                "fn",
                "public",
                "private",
            ]
            .contains(&self.peek().lexeme.as_str())
            {
                return;
            }
            self.advance();
        }
    }

    fn synchronize_member(&mut self) {
        while !self.is_at_end() && !self.check_symbol("}") {
            if self.previous().lexeme == ";" || self.check_symbol("{") {
                break;
            }
            self.advance();
        }
        if self.check_symbol("{") {
            let mut depth = 0usize;
            while !self.is_at_end() {
                let token = self.advance();
                if token.lexeme == "{" {
                    depth += 1;
                }
                if token.lexeme == "}" {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
            }
        } else if self.check_symbol(";") {
            self.advance();
        }
    }

    fn synchronize_statement(&mut self) {
        while !self.is_at_end() && !self.check_symbol("}") {
            if self.previous().lexeme == ";" {
                return;
            }
            self.advance();
        }
    }
}

fn infix_precedence(operator: &str) -> Option<(u8, bool, bool)> {
    Some(match operator {
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" => {
            (1, true, true)
        }
        "||" => (2, false, false),
        "&&" => (3, false, false),
        "|" => (4, false, false),
        "^" => (5, false, false),
        "&" => (6, false, false),
        "==" | "!=" => (7, false, false),
        "<" | ">" | "<=" | ">=" => (8, false, false),
        "<<" | ">>" => (9, false, false),
        "+" | "-" => (10, false, false),
        "*" | "/" | "%" => (11, false, false),
        _ => return None,
    })
}

fn unescape_character(raw: &str) -> char {
    let inner = raw.get(1..raw.len().saturating_sub(1)).unwrap_or("");
    match inner {
        r"\n" => '\n',
        r"\r" => '\r',
        r"\t" => '\t',
        r"\0" => '\0',
        r"\\" => '\\',
        r"\'" => '\'',
        value => value.chars().next().unwrap_or('\0'),
    }
}

fn unescape_string(raw: &str) -> String {
    let inner = raw.strip_prefix('"').unwrap_or(raw);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    let mut output = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => output.push('\n'),
                Some('r') => output.push('\r'),
                Some('t') => output.push('\t'),
                Some('0') => output.push('\0'),
                Some('"') => output.push('"'),
                Some('\\') => output.push('\\'),
                Some(other) => {
                    output.push('\\');
                    output.push(other);
                }
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn join_span(start: SourceSpan, end: SourceSpan) -> SourceSpan {
    SourceSpan {
        start: start.start,
        end: end.end,
    }
}

fn statement_span(statement: &Statement) -> SourceSpan {
    match statement {
        Statement::Block(block) => block.span,
        Statement::Variable(value) => value.span,
        Statement::Return(_, span)
        | Statement::Throw(_, span)
        | Statement::Expression(_, span)
        | Statement::Break(span)
        | Statement::Continue(span)
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

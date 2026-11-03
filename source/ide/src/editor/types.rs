//! Typauflösung für IntelliSense (Membervorschläge, abgeleitete Variablentypen).
//!
//! Die Regeln beschränken sich auf Fälle, deren Ergebnis eindeutig aus den
//! Deklarationen folgt; ist ein Typ nicht sicher bestimmbar (Generics, überladene
//! Funktionen mit verschiedenen Rückgabetypen, unbekannte Namen), wird `None`
//! geliefert statt zu raten. Typen werden als Text geführt (`Name<Args>[]`).

use std::collections::{HashMap, HashSet};

use nortex_compiler::ast::{
    ClassDeclaration, Declaration, Expression, FieldDeclaration, FunctionDeclaration,
    MemberDeclaration, Program, TypeReference,
};
use nortex_compiler::lexer::{Token, TokenKind};

pub struct TypeEnv<'a> {
    classes: HashMap<&'a str, &'a ClassDeclaration>,
    functions: HashMap<&'a str, Vec<&'a FunctionDeclaration>>,
    globals: HashMap<&'a str, &'a FieldDeclaration>,
}

/// Auflösungskontext an der Cursorposition. `locals` liefert `Some(None)` für einen
/// lokalen Namen mit unbekanntem Typ; er verdeckt dann gleichnamige globale Symbole.
pub struct Scope<'a> {
    pub locals: &'a dyn Fn(&str) -> Option<Option<String>>,
    pub class: Option<&'a str>,
}

pub enum Member<'a> {
    Field(&'a FieldDeclaration),
    Method(&'a FunctionDeclaration),
    Variant(&'a str),
}

pub fn type_text(type_ref: &TypeReference) -> String {
    let mut text = type_ref.name.clone();
    if !type_ref.generic_arguments.is_empty() {
        let arguments: Vec<String> = type_ref.generic_arguments.iter().map(type_text).collect();
        text.push('<');
        text.push_str(&arguments.join(", "));
        text.push('>');
    }
    if type_ref.fixed_array_length.is_some() {
        text.push_str("[]");
    }
    text.push_str(&"[]".repeat(type_ref.array_depth));
    text
}

/// Klassenname ohne Generic-Argumente; `None` für Arraytypen.
fn class_name(type_text: &str) -> Option<&str> {
    if type_text.ends_with(']') {
        return None;
    }
    Some(type_text.split('<').next().unwrap_or(type_text))
}

impl<'a> TypeEnv<'a> {
    /// Spätere Programme überschreiben frühere gleichnamige Deklarationen; die aktive
    /// Datei gehört daher ans Ende. Doppelte Namen meldet der Compiler als Fehler.
    pub fn new(programs: impl IntoIterator<Item = &'a Program>) -> Self {
        let mut env = Self {
            classes: HashMap::new(),
            functions: HashMap::new(),
            globals: HashMap::new(),
        };
        for program in programs {
            for declaration in &program.declarations {
                match declaration {
                    Declaration::Class(class) => {
                        env.classes.insert(&class.name, class);
                    }
                    Declaration::Function(function) => {
                        env.functions
                            .entry(&function.name)
                            .or_default()
                            .push(function);
                    }
                    Declaration::GlobalVariable(global) => {
                        env.globals.insert(&global.name, global);
                    }
                }
            }
        }
        env
    }

    pub fn class(&self, name: &str) -> Option<&'a ClassDeclaration> {
        self.classes.get(name).copied()
    }

    /// Die zuletzt deklarierte freie Funktion dieses Namens.
    pub fn function(&self, name: &str) -> Option<&'a FunctionDeclaration> {
        self.functions.get(name)?.last().copied()
    }

    pub fn class_of_type(&self, type_text: &str) -> Option<&'a ClassDeclaration> {
        self.class(class_name(type_text)?)
    }

    /// Die Klasse selbst und alle Basistypen (Breitensuche, zyklensicher).
    fn hierarchy(&self, class: &'a ClassDeclaration) -> Vec<&'a ClassDeclaration> {
        let mut order = vec![class];
        let mut seen = HashSet::from([class.name.as_str()]);
        let mut index = 0;
        while index < order.len() {
            for base in &order[index].base_types {
                if let Some(base_class) = self.class(&base.name)
                    && seen.insert(base_class.name.as_str())
                {
                    order.push(base_class);
                }
            }
            index += 1;
        }
        order
    }

    /// Sichtbare Member: Basisklassen-Member nur, wenn nicht privat; private Member
    /// der Klasse selbst nur innerhalb dieser Klasse (wie der Typprüfer des Compilers).
    /// Methoden gelten je Name und Parameteranzahl als eine Überladung; die
    /// abgeleitete Klasse verdeckt gleichartige Member der Basis.
    pub fn members(&self, class: &'a ClassDeclaration, inside: Option<&str>) -> Vec<Member<'a>> {
        let mut members = Vec::new();
        let mut keys = HashSet::new();
        for (depth, declaring) in self.hierarchy(class).into_iter().enumerate() {
            let own_class = depth == 0 && inside == Some(declaring.name.as_str());
            let mut declared_here = Vec::new();
            for member in &declaring.members {
                let (key, member, modifiers) = match member {
                    MemberDeclaration::Field(field) => {
                        (field.name.clone(), Member::Field(field), &field.modifiers)
                    }
                    MemberDeclaration::Method(method) => (
                        format!("{}/{}", method.name, method.parameters.len()),
                        Member::Method(method),
                        &method.modifiers,
                    ),
                    MemberDeclaration::Variant(variant) if depth == 0 => {
                        members.push(Member::Variant(&variant.name));
                        continue;
                    }
                    MemberDeclaration::Variant(_) | MemberDeclaration::Operator(_) => continue,
                };
                if modifiers.iter().any(|modifier| modifier == "private") && !own_class {
                    continue;
                }
                if !keys.contains(&key) {
                    declared_here.push(key);
                    members.push(member);
                }
            }
            keys.extend(declared_here);
        }
        members
    }

    fn member_type(
        &self,
        class: &'a ClassDeclaration,
        name: &str,
        arguments: Option<usize>,
        inside: Option<&str>,
    ) -> Option<String> {
        let mut types = Vec::new();
        for member in self.members(class, inside) {
            match member {
                Member::Field(field) if arguments.is_none() && field.name == name => {
                    types.push((&field.type_ref, &class.generic_parameters, None));
                }
                Member::Method(method)
                    if method.name == name && arguments == Some(method.parameters.len()) =>
                {
                    types.push((&method.return_type, &class.generic_parameters, Some(method)));
                }
                Member::Variant(variant) if arguments.is_none() && variant == name => {
                    return Some(class.name.clone());
                }
                _ => {}
            }
        }
        let mut texts = types.into_iter().map(|(type_ref, class_generics, method)| {
            let is_generic = class_generics.contains(&type_ref.name)
                || method.is_some_and(|method| method.generic_parameters.contains(&type_ref.name));
            (!is_generic).then(|| type_text(type_ref))
        });
        let first = texts.next()??;
        (texts.all(|other| other.as_ref() == Some(&first)) && first != "void").then_some(first)
    }

    fn function_return(&self, name: &str, arguments: usize) -> Option<String> {
        let mut returns = self
            .functions
            .get(name)?
            .iter()
            .filter(|function| function.parameters.len() == arguments)
            .filter(|function| {
                !function
                    .generic_parameters
                    .contains(&function.return_type.name)
            })
            .map(|function| type_text(&function.return_type));
        let first = returns.next()?;
        (returns.all(|other| other == first) && first != "void").then_some(first)
    }

    fn name_type(&self, name: &str, scope: &Scope) -> Option<String> {
        if let Some(local) = (scope.locals)(name) {
            return local;
        }
        if let Some(class) = scope.class.and_then(|class| self.class(class))
            && let Some(field) = self.member_type(class, name, None, scope.class)
        {
            return Some(field);
        }
        if let Some(global) = self.globals.get(name) {
            return Some(type_text(&global.type_ref));
        }
        self.classes.contains_key(name).then(|| name.to_owned())
    }

    fn call_type(&self, name: &str, arguments: usize, scope: &Scope) -> Option<String> {
        if let Some(class) = scope.class.and_then(|class| self.class(class))
            && let Some(result) = self.member_type(class, name, Some(arguments), scope.class)
        {
            return Some(result);
        }
        self.function_return(name, arguments)
    }

    fn member_of(
        &self,
        owner: &str,
        name: &str,
        arguments: Option<usize>,
        scope: &Scope,
    ) -> Option<String> {
        let class = self.class_of_type(owner)?;
        self.member_type(class, name, arguments, scope.class)
    }

    /// Typ eines Ausdrucks aus dem AST (Initialisierer von `var`-Deklarationen).
    pub fn expression_type(&self, expression: &Expression, scope: &Scope) -> Option<String> {
        match expression {
            Expression::Integer(_, _) => Some("int".into()),
            Expression::Float(_, _) => Some("float".into()),
            Expression::String(_, _) => Some("string".into()),
            Expression::Character(_, _) => Some("char".into()),
            Expression::Boolean(_, _) => Some("bool".into()),
            Expression::Null(_) => None,
            Expression::Identifier(name, _) if name == "this" => scope.class.map(str::to_owned),
            Expression::Identifier(name, _) => self.name_type(name, scope),
            Expression::ObjectCreation { type_ref, .. } => Some(type_text(type_ref)),
            Expression::Group(inner, _) => self.expression_type(inner, scope),
            Expression::Unary {
                operator, operand, ..
            } => {
                if operator == "!" {
                    Some("bool".into())
                } else {
                    self.expression_type(operand, scope)
                }
            }
            Expression::Binary {
                left,
                operator,
                right,
                ..
            } => {
                if matches!(
                    operator.as_str(),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
                ) {
                    return Some("bool".into());
                }
                let left_type = self.expression_type(left, scope)?;
                let right_type = self.expression_type(right, scope)?;
                if operator == "+" && (left_type == "string" || right_type == "string") {
                    Some("string".into())
                } else if left_type == "float" || right_type == "float" {
                    Some("float".into())
                } else if left_type == right_type {
                    Some(left_type)
                } else {
                    None
                }
            }
            Expression::Conditional {
                when_true,
                when_false,
                ..
            } => {
                let true_type = self.expression_type(when_true, scope)?;
                (self.expression_type(when_false, scope).as_deref() == Some(&true_type))
                    .then_some(true_type)
            }
            Expression::Assignment { value, .. } => self.expression_type(value, scope),
            Expression::Call {
                callee, arguments, ..
            } => match callee.as_ref() {
                Expression::Identifier(name, _) => self.call_type(name, arguments.len(), scope),
                Expression::Member { object, name, .. } => {
                    let owner = self.expression_type(object, scope)?;
                    self.member_of(&owner, name, Some(arguments.len()), scope)
                }
                _ => None,
            },
            Expression::Member { object, name, .. } => {
                let owner = self.expression_type(object, scope)?;
                self.member_of(&owner, name, None, scope)
            }
            Expression::Index { object, .. } => {
                let owner = self.expression_type(object, scope)?;
                owner.strip_suffix("[]").map(str::to_owned)
            }
        }
    }

    /// Typ des Ausdrucks, der unmittelbar vor `dot` (Index des `.`-Tokens) endet.
    /// Funktioniert auf Token-Ebene, damit auch unvollständiger Code ausgewertet wird.
    pub fn receiver_type(&self, tokens: &[Token], dot: usize, scope: &Scope) -> Option<String> {
        let mut steps = chain_before(tokens, dot)?.into_iter().peekable();
        let mut current = match steps.next()? {
            Step::New(type_name) => type_name,
            Step::Name(name) if name == "this" => scope.class?.to_owned(),
            Step::Name(name) => match steps.next_if(|step| matches!(step, Step::Call(_))) {
                Some(Step::Call(arguments)) => self.call_type(&name, arguments, scope)?,
                _ => self.name_type(&name, scope)?,
            },
            Step::Call(_) | Step::Index => return None,
        };
        while let Some(step) = steps.next() {
            current = match step {
                Step::Name(name) => match steps.next_if(|next| matches!(next, Step::Call(_))) {
                    Some(Step::Call(arguments)) => {
                        self.member_of(&current, &name, Some(arguments), scope)?
                    }
                    _ => self.member_of(&current, &name, None, scope)?,
                },
                Step::Index => current.strip_suffix("[]")?.to_owned(),
                Step::Call(_) | Step::New(_) => return None,
            };
        }
        Some(current)
    }
}

enum Step {
    Name(String),
    Call(usize),
    Index,
    New(String),
}

/// Zerlegt die Kette vor `dot` (z. B. `make().items[0]`) rückwärts in Schritte.
fn chain_before(tokens: &[Token], dot: usize) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let mut position = dot;
    loop {
        let token = tokens.get(position.checked_sub(1)?)?;
        match &token.kind {
            TokenKind::Symbol(symbol) if symbol == ")" || symbol == "]" => {
                let (open, close) = if symbol == ")" {
                    ("(", ")")
                } else {
                    ("[", "]")
                };
                let start = matching_open(tokens, position - 1, open, close)?;
                if symbol == ")" {
                    steps.push(Step::Call(argument_count(&tokens[start + 1..position - 1])));
                } else {
                    steps.push(Step::Index);
                }
                position = start;
                // `new Type<Args>(...)`: die Typargumente werden übersprungen.
                if symbol == ")"
                    && position > 0
                    && is_symbol(&tokens[position - 1], ">")
                    && let Some(open_angle) = matching_open(tokens, position - 1, "<", ">")
                    && open_angle >= 2
                    && tokens[open_angle - 2].lexeme == "new"
                {
                    position = open_angle;
                }
                continue;
            }
            TokenKind::Identifier | TokenKind::Keyword => {
                let name = token.lexeme.clone();
                position -= 1;
                let before = position.checked_sub(1).and_then(|index| tokens.get(index));
                match before {
                    Some(previous) if is_symbol(previous, ".") => {
                        steps.push(Step::Name(name));
                        position -= 1;
                    }
                    Some(previous) if previous.lexeme == "new" => {
                        // `new Type(...)`: der Aufruf-Schritt gehört zur Konstruktion.
                        if matches!(steps.last(), Some(Step::Call(_))) {
                            steps.pop();
                        }
                        steps.push(Step::New(name));
                        break;
                    }
                    _ => {
                        steps.push(Step::Name(name));
                        break;
                    }
                }
            }
            _ => return None,
        }
    }
    steps.reverse();
    Some(steps)
}

fn is_symbol(token: &Token, expected: &str) -> bool {
    matches!(&token.kind, TokenKind::Symbol(symbol) if symbol == expected)
}

fn matching_open(
    tokens: &[Token],
    close: usize,
    open_symbol: &str,
    close_symbol: &str,
) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..=close).rev() {
        if is_symbol(&tokens[index], close_symbol) {
            depth += 1;
        } else if is_symbol(&tokens[index], open_symbol) {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn argument_count(tokens: &[Token]) -> usize {
    if tokens.is_empty() {
        return 0;
    }
    let mut depth = 0usize;
    let mut commas = 0;
    for token in tokens {
        if let TokenKind::Symbol(symbol) = &token.kind {
            match symbol.as_str() {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                "," if depth == 0 => commas += 1,
                _ => {}
            }
        }
    }
    commas + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use nortex_compiler::ast::Statement;
    use nortex_compiler::lexer::Lexer;
    use nortex_compiler::parser::Parser;

    fn parse(source: &str) -> Program {
        Parser::new(Lexer::new(source).tokenize().tokens)
            .parse()
            .program
    }

    fn no_locals(_: &str) -> Option<Option<String>> {
        None
    }

    fn scope<'a>(class: Option<&'a str>) -> Scope<'a> {
        Scope {
            locals: &no_locals,
            class,
        }
    }

    /// Initialisierer der ersten Variablendeklaration im Rumpf von `f`.
    fn initializer(program: &Program) -> &Expression {
        let Declaration::Function(function) = &program.declarations[0] else {
            panic!("keine Funktion");
        };
        let Statement::Variable(variable) = &function.body.as_ref().unwrap().statements[0] else {
            panic!("keine Variable");
        };
        variable.initializer.as_ref().unwrap()
    }

    fn expression_type_of(env: &TypeEnv, expression: &str) -> Option<String> {
        let program = parse(&format!("public void f() {{ var v = {expression}; }}"));
        env.expression_type(initializer(&program), &scope(None))
    }

    fn receiver_of(env: &TypeEnv, text: &str, class: Option<&str>) -> Option<String> {
        let tokens = Lexer::new(text).tokenize().tokens;
        let dot = tokens
            .iter()
            .rposition(|token| is_symbol(token, "."))
            .expect("Text endet auf einen Punkt");
        env.receiver_type(&tokens, dot, &scope(class))
    }

    const SHOP: &str = "
        public class Item { public string title; public int price() { return 1; } private int secret; }
        public class Cart { public Item first; public Item[] items; public List<int> ids; public void clear() { } }
        public Cart open() { return new Cart(); }
        public int total(int a, int b) { return a + b; }
        public enum Color { Red, Green }
        Cart current;
    ";

    fn member_names(env: &TypeEnv, class: &str, inside: Option<&str>) -> Vec<String> {
        env.members(env.class(class).unwrap(), inside)
            .into_iter()
            .map(|member| match member {
                Member::Field(field) => field.name.clone(),
                Member::Method(method) => method.name.clone(),
                Member::Variant(name) => name.to_owned(),
            })
            .collect()
    }

    #[test]
    fn type_text_covers_generics_and_arrays() {
        let program = parse(
            "class A { public List<int> a; public Dictionary<string, List<int>> b; public int[] c; public int[][] d; }",
        );
        let Declaration::Class(class) = &program.declarations[0] else {
            panic!();
        };
        let texts: Vec<String> = class
            .members
            .iter()
            .map(|member| match member {
                MemberDeclaration::Field(field) => type_text(&field.type_ref),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            texts,
            [
                "List<int>",
                "Dictionary<string, List<int>>",
                "int[]",
                "int[][]"
            ]
        );
    }

    #[test]
    fn literals_and_operators_have_the_compilers_basic_types() {
        let env = TypeEnv::new([]);
        let cases = [
            ("1", Some("int")),
            ("1.5", Some("float")),
            ("\"a\"", Some("string")),
            ("'c'", Some("char")),
            ("true", Some("bool")),
            ("1 + 2", Some("int")),
            ("1 + 2.5", Some("float")),
            ("\"a\" + 1", Some("string")),
            ("1 < 2", Some("bool")),
            ("1 == 2 && true", Some("bool")),
            ("!true", Some("bool")),
            ("(1 + 2)", Some("int")),
            ("true ? 1 : 2", Some("int")),
            ("true ? 1 : \"x\"", None),
            ("null", None),
            ("1 + true", None),
        ];
        for (source, expected) in cases {
            assert_eq!(
                expression_type_of(&env, source).as_deref(),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn empty_environment_resolves_nothing() {
        let env = TypeEnv::new([]);
        assert_eq!(expression_type_of(&env, "unknown()"), None);
        assert_eq!(expression_type_of(&env, "unknown"), None);
        assert_eq!(expression_type_of(&env, "unknown.member"), None);
        assert_eq!(receiver_of(&env, "unknown.", None), None);
        assert!(env.class("Cart").is_none());
        assert!(env.function("total").is_none());
        assert!(env.class_of_type("Cart").is_none());
    }

    #[test]
    fn declarations_drive_calls_members_and_chains() {
        let program = parse(SHOP);
        let env = TypeEnv::new([&program]);
        assert_eq!(expression_type_of(&env, "open()").as_deref(), Some("Cart"));
        assert_eq!(
            expression_type_of(&env, "total(1, 2)").as_deref(),
            Some("int")
        );
        assert_eq!(expression_type_of(&env, "total(1)"), None);
        assert_eq!(
            expression_type_of(&env, "new Cart()").as_deref(),
            Some("Cart")
        );
        assert_eq!(expression_type_of(&env, "current").as_deref(), Some("Cart"));
        assert_eq!(
            expression_type_of(&env, "current.first").as_deref(),
            Some("Item")
        );
        assert_eq!(
            expression_type_of(&env, "open().first.title").as_deref(),
            Some("string")
        );
        assert_eq!(
            expression_type_of(&env, "open().first.price()").as_deref(),
            Some("int")
        );
        assert_eq!(expression_type_of(&env, "open().first.price"), None);
        assert_eq!(expression_type_of(&env, "open().missing"), None);
        assert_eq!(expression_type_of(&env, "open().clear()"), None);
    }

    #[test]
    fn token_chains_resolve_like_the_ast() {
        let program = parse(SHOP);
        let env = TypeEnv::new([&program]);
        let cases = [
            ("current.", Some("Cart")),
            ("current.first.", Some("Item")),
            ("open().first.", Some("Item")),
            ("new Cart().first.", Some("Item")),
            ("current.items[0].", Some("Item")),
            ("current.items.", Some("Item[]")),
            ("current.ids.", Some("List<int>")),
            ("Color.Red.", Some("Color")),
            ("Cart.", Some("Cart")),
            ("current.missing.", None),
            ("current.first.price().", Some("int")),
            ("open(1).", None),
            ("current.items[0][1].", None),
        ];
        for (text, expected) in cases {
            assert_eq!(receiver_of(&env, text, None).as_deref(), expected, "{text}");
        }
    }

    #[test]
    fn incomplete_or_broken_chains_yield_nothing() {
        let program = parse(SHOP);
        let env = TypeEnv::new([&program]);
        for text in [
            "open(.",
            "open().first(.",
            "current[.",
            "].",
            ")",
            "(1 + 2).",
            ".",
            "new .",
        ] {
            let tokens = Lexer::new(text).tokenize().tokens;
            let dot = tokens.iter().rposition(|token| is_symbol(token, "."));
            if let Some(dot) = dot {
                assert_eq!(
                    env.receiver_type(&tokens, dot, &scope(None)),
                    None,
                    "{text}"
                );
            }
        }
        assert_eq!(env.receiver_type(&[], 0, &scope(None)), None);
        assert_eq!(env.receiver_type(&[], 5, &scope(None)), None);
    }

    #[test]
    fn this_and_own_members_need_the_enclosing_class() {
        let program = parse(SHOP);
        let env = TypeEnv::new([&program]);
        assert_eq!(
            receiver_of(&env, "this.", Some("Cart")).as_deref(),
            Some("Cart")
        );
        assert_eq!(
            receiver_of(&env, "this.first.", Some("Cart")).as_deref(),
            Some("Item")
        );
        assert_eq!(receiver_of(&env, "this.", None), None);
        assert_eq!(
            receiver_of(&env, "first.", Some("Cart")).as_deref(),
            Some("Item")
        );
        assert_eq!(receiver_of(&env, "first.", None), None);
    }

    #[test]
    fn private_members_are_only_visible_inside_their_class() {
        let program = parse(SHOP);
        let env = TypeEnv::new([&program]);
        assert!(member_names(&env, "Item", Some("Item")).contains(&"secret".to_owned()));
        assert!(!member_names(&env, "Item", Some("Cart")).contains(&"secret".to_owned()));
        assert!(!member_names(&env, "Item", None).contains(&"secret".to_owned()));
        assert_eq!(
            env.expression_type(
                initializer(&parse("public void f() { var v = current.first.secret; }")),
                &scope(None)
            ),
            None,
            "private Felder bleiben für Fremde unaufgelöst"
        );
    }

    #[test]
    fn inheritance_adds_visible_base_members_and_derived_members_win() {
        let program = parse(
            "public class Base { public int shared() { return 1; } public string label; private int hidden; }
             public class Child : Base { public string shared() { return \"x\"; } public int extra; }",
        );
        let env = TypeEnv::new([&program]);
        let names = member_names(&env, "Child", Some("Child"));
        assert_eq!(names.iter().filter(|name| *name == "shared").count(), 1);
        assert!(names.contains(&"label".to_owned()));
        assert!(names.contains(&"extra".to_owned()));
        assert!(!names.contains(&"hidden".to_owned()));
        assert_eq!(
            env.expression_type(
                initializer(&parse("public void f() { var v = new Child().shared(); }")),
                &scope(None)
            )
            .as_deref(),
            Some("string")
        );
        assert_eq!(
            receiver_of(&env, "new Child().label.", None).as_deref(),
            Some("string")
        );
    }

    #[test]
    fn circular_inheritance_terminates() {
        let program =
            parse("public class A : B { public int a; } public class B : A { public int b; }");
        let env = TypeEnv::new([&program]);
        let mut names = member_names(&env, "A", Some("A"));
        names.sort();
        assert_eq!(names, ["a", "b"]);
        let self_base = parse("public class S : S { public int s; }");
        let env = TypeEnv::new([&self_base]);
        assert_eq!(member_names(&env, "S", Some("S")), ["s"]);
    }

    #[test]
    fn enum_variants_resolve_to_their_enum() {
        let program = parse(SHOP);
        let env = TypeEnv::new([&program]);
        assert_eq!(member_names(&env, "Color", None), ["Red", "Green"]);
        assert_eq!(
            expression_type_of(&env, "Color.Red").as_deref(),
            Some("Color")
        );
        assert_eq!(expression_type_of(&env, "Color.Blue"), None);
    }

    #[test]
    fn overloads_and_generics_are_only_resolved_when_unambiguous() {
        let program = parse(
            "public int same(int a) { return 1; } public int same(string a) { return 2; }
             public int mixed(int a) { return 1; } public string mixed(string a) { return \"\"; }
             public class Box<T> { public T value; public T get() { return value; } public List<T> all() { return null; } }
             public T identity<T>(T input) { return input; }
             public void nothing() { }",
        );
        let env = TypeEnv::new([&program]);
        assert_eq!(expression_type_of(&env, "same(1)").as_deref(), Some("int"));
        assert_eq!(expression_type_of(&env, "mixed(1)"), None);
        assert_eq!(expression_type_of(&env, "identity(1)"), None);
        assert_eq!(expression_type_of(&env, "nothing()"), None);
        assert_eq!(receiver_of(&env, "new Box<int>().value.", None), None);
        assert_eq!(receiver_of(&env, "new Box<int>().get().", None), None);
        assert_eq!(
            receiver_of(&env, "new Box<int>().", None).as_deref(),
            Some("Box")
        );
        assert!(env.class_of_type("Box<int>").is_some());
        assert!(env.class_of_type("Box<int>[]").is_none());
    }

    #[test]
    fn arrays_are_indexed_but_have_no_members() {
        let program = parse("public class P { public int[] values; public int[][] grid; }");
        let env = TypeEnv::new([&program]);
        assert_eq!(
            receiver_of(&env, "new P().values[0].", None).as_deref(),
            Some("int")
        );
        assert_eq!(
            receiver_of(&env, "new P().grid[0].", None).as_deref(),
            Some("int[]")
        );
        assert_eq!(
            receiver_of(&env, "new P().grid[0][1].", None).as_deref(),
            Some("int")
        );
        assert!(env.class_of_type("int[]").is_none());
        assert_eq!(receiver_of(&env, "new P().values[0][1].", None), None);
    }

    #[test]
    fn locals_shadow_globals_even_with_unknown_types() {
        let program = parse("public class Cart { public int n; } Cart current;");
        let env = TypeEnv::new([&program]);
        let unknown = |name: &str| (name == "current").then_some(None);
        let known = |name: &str| (name == "current").then(|| Some("int".to_owned()));
        let scope_with = |lookup: &dyn Fn(&str) -> Option<Option<String>>| {
            let tokens = Lexer::new("current.").tokenize().tokens;
            let scope = Scope {
                locals: lookup,
                class: None,
            };
            env.receiver_type(&tokens, 1, &scope)
        };
        assert_eq!(scope_with(&no_locals).as_deref(), Some("Cart"));
        assert_eq!(scope_with(&unknown), None);
        assert_eq!(scope_with(&known).as_deref(), Some("int"));
    }

    #[test]
    fn later_programs_override_earlier_declarations() {
        let imported = parse("public class Shape { public int sides; }");
        let active = parse("public class Shape { public string name; }");
        let env = TypeEnv::new([&imported, &active]);
        assert_eq!(member_names(&env, "Shape", None), ["name"]);
        let env = TypeEnv::new([&active, &imported]);
        assert_eq!(member_names(&env, "Shape", None), ["sides"]);
    }

    #[test]
    fn duplicate_functions_with_different_returns_stay_ambiguous() {
        let first = parse("public int make() { return 1; }");
        let second = parse("public string make() { return \"\"; }");
        let env = TypeEnv::new([&first, &second]);
        assert_eq!(expression_type_of(&env, "make()"), None);
        assert_eq!(
            env.function("make")
                .map(|f| type_text(&f.return_type))
                .as_deref(),
            Some("string")
        );
    }

    #[test]
    fn queries_are_repeatable_and_a_rebuilt_env_sees_changed_declarations() {
        let before = parse("public class Cart { public int count; } Cart current;");
        let env = TypeEnv::new([&before]);
        for _ in 0..3 {
            assert_eq!(
                receiver_of(&env, "current.count.", None).as_deref(),
                Some("int")
            );
        }
        let after = parse("public class Cart { public string count; } Cart current;");
        let env = TypeEnv::new([&after]);
        assert_eq!(
            receiver_of(&env, "current.count.", None).as_deref(),
            Some("string")
        );
        let removed = parse("Cart current;");
        let env = TypeEnv::new([&removed]);
        assert_eq!(receiver_of(&env, "current.count.", None), None);
    }

    #[test]
    fn broken_declarations_do_not_panic() {
        let program = parse("public class { public int ; } public int f( { var x = ; Cart c");
        let env = TypeEnv::new([&program]);
        assert_eq!(receiver_of(&env, "c.", None), None);
        let _ = member_names_of_all(&env);
    }

    fn member_names_of_all(env: &TypeEnv) -> usize {
        env.classes
            .values()
            .map(|class| env.members(class, Some(&class.name)).len())
            .sum()
    }
}

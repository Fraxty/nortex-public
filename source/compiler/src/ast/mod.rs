//! Abstract syntax tree for the first NORTEX language subset.

use crate::diagnostics::SourceSpan;

mod shift;
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub usings: Vec<UsingDirective>,
    pub namespace: Option<Vec<String>>,
    pub declarations: Vec<Declaration>,
}

impl Program {
    pub fn empty() -> Self {
        Self {
            usings: Vec::new(),
            namespace: None,
            declarations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsingDirective {
    pub path: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    Class(ClassDeclaration),
    Function(FunctionDeclaration),
    GlobalVariable(FieldDeclaration),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDeclaration {
    pub kind: TypeDeclarationKind,
    pub name: String,
    pub generic_parameters: Vec<String>,
    pub modifiers: Vec<String>,
    pub base_types: Vec<TypeReference>,
    pub members: Vec<MemberDeclaration>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeDeclarationKind {
    Class,
    Struct,
    Record,
    Interface,
    Trait,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemberDeclaration {
    Field(FieldDeclaration),
    Method(FunctionDeclaration),
    Operator(OperatorDeclaration),
    Variant(EnumVariant),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub payload_types: Vec<TypeReference>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDeclaration {
    pub name: String,
    pub type_ref: TypeReference,
    pub modifiers: Vec<String>,
    pub initializer: Option<Expression>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub generic_parameters: Vec<String>,
    pub return_type: TypeReference,
    pub parameters: Vec<Parameter>,
    pub modifiers: Vec<String>,
    pub body: Option<BlockStatement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatorDeclaration {
    pub operator: String,
    pub return_type: TypeReference,
    pub parameters: Vec<Parameter>,
    pub modifiers: Vec<String>,
    pub body: Option<BlockStatement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub type_ref: TypeReference,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeReference {
    pub name: String,
    pub generic_arguments: Vec<TypeReference>,
    /// Number of dynamic array suffixes (`T[]`).
    pub array_depth: usize,
    /// Fixed array length for `[T; N]`.
    pub fixed_array_length: Option<usize>,
    /// Borrow or raw-pointer qualifier applied to the type.
    pub reference_kind: ReferenceKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Value,
    SharedBorrow,
    MutableBorrow,
    RawConstPointer,
    RawMutablePointer,
}

impl TypeReference {
    pub fn simple(name: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            name: name.into(),
            generic_arguments: Vec::new(),
            array_depth: 0,
            fixed_array_length: None,
            reference_kind: ReferenceKind::Value,
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Block(BlockStatement),
    Variable(VariableDeclaration),
    Return(Option<Expression>, SourceSpan),
    If {
        condition: Expression,
        then_branch: Box<Statement>,
        else_branch: Option<Box<Statement>>,
        span: SourceSpan,
    },
    While {
        condition: Expression,
        body: Box<Statement>,
        span: SourceSpan,
    },
    For {
        initializer: Option<Box<Statement>>,
        condition: Option<Expression>,
        increment: Option<Expression>,
        body: Box<Statement>,
        span: SourceSpan,
    },
    ForEach {
        variable_type: TypeReference,
        variable_name: String,
        iterable: Expression,
        body: Box<Statement>,
        span: SourceSpan,
    },
    Switch {
        value: Expression,
        cases: Vec<SwitchCase>,
        span: SourceSpan,
    },
    Throw(Option<Expression>, SourceSpan),
    Break(SourceSpan),
    Continue(SourceSpan),
    Try {
        body: BlockStatement,
        catches: Vec<CatchClause>,
        finally_block: Option<BlockStatement>,
        span: SourceSpan,
    },
    Match {
        value: Expression,
        arms: Vec<MatchArm>,
        span: SourceSpan,
    },
    Expression(Expression, SourceSpan),
    Empty(SourceSpan),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableDeclaration {
    pub name: String,
    pub type_ref: Option<TypeReference>,
    pub initializer: Option<Expression>,
    pub mutability: VariableMutability,
    pub span: SourceSpan,
}

/// Mutability syntax. Unqualified bindings are immutable; `mut` opts in to
/// mutation and `const` denotes a compile-time constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableMutability {
    Default,
    Mutable,
    Immutable,
    CompileTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub exception_type: Option<TypeReference>,
    pub variable_name: Option<String>,
    pub body: BlockStatement,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Enum variant name, or `_` for the fallback arm.
    pub variant: String,
    /// Optional single binding for tuple-style enum payloads.
    pub binding: Option<String>,
    pub body: BlockStatement,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    /// `None` marks the default arm.
    pub value: Option<Expression>,
    pub statements: Vec<Statement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Identifier(String, SourceSpan),
    Integer(String, SourceSpan),
    Float(String, SourceSpan),
    String(String, SourceSpan),
    Character(char, SourceSpan),
    Boolean(bool, SourceSpan),
    Null(SourceSpan),
    Unary {
        operator: String,
        operand: Box<Expression>,
        span: SourceSpan,
    },
    Binary {
        left: Box<Expression>,
        operator: String,
        right: Box<Expression>,
        span: SourceSpan,
    },
    Conditional {
        condition: Box<Expression>,
        when_true: Box<Expression>,
        when_false: Box<Expression>,
        span: SourceSpan,
    },
    Assignment {
        target: Box<Expression>,
        operator: String,
        value: Box<Expression>,
        span: SourceSpan,
    },
    Call {
        callee: Box<Expression>,
        generic_arguments: Vec<TypeReference>,
        arguments: Vec<Expression>,
        span: SourceSpan,
    },
    ObjectCreation {
        type_ref: TypeReference,
        arguments: Vec<Expression>,
        span: SourceSpan,
    },
    Member {
        object: Box<Expression>,
        name: String,
        span: SourceSpan,
    },
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
        span: SourceSpan,
    },
    Group(Box<Expression>, SourceSpan),
}

impl Expression {
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Identifier(_, span)
            | Self::Integer(_, span)
            | Self::Float(_, span)
            | Self::String(_, span)
            | Self::Character(_, span)
            | Self::Boolean(_, span)
            | Self::Null(span)
            | Self::Group(_, span) => *span,
            Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Conditional { span, .. }
            | Self::Assignment { span, .. }
            | Self::Call { span, .. }
            | Self::ObjectCreation { span, .. }
            | Self::Member { span, .. }
            | Self::Index { span, .. } => *span,
        }
    }
}

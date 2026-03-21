//! AST 定義。

/// tiny-lang 程式。
pub type Program = Vec<Statement>;

/// 型別註記。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeAnnotation {
    Int,
    Str,
    Bool,
    ArrayOf(Box<TypeAnnotation>),
    MapOf(Box<TypeAnnotation>),
    Named(String),
    Any,
}

impl TypeAnnotation {
    pub fn display_name(&self) -> String {
        match self {
            TypeAnnotation::Int => "int".into(),
            TypeAnnotation::Str => "str".into(),
            TypeAnnotation::Bool => "bool".into(),
            TypeAnnotation::ArrayOf(inner) => format!("[{}]", inner.display_name()),
            TypeAnnotation::MapOf(inner) => format!("{{{}}}", inner.display_name()),
            TypeAnnotation::Named(name) => name.clone(),
            TypeAnnotation::Any => "any".into(),
        }
    }
}

/// enum variant AST。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Option<Vec<(String, Option<TypeAnnotation>)>>,
}

/// interface method AST
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<(String, Option<TypeAnnotation>)>,
    pub return_type: Option<TypeAnnotation>,
}

/// 陳述式。
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Import { path: String },
    StructDecl {
        name: String,
        fields: Vec<(String, Option<TypeAnnotation>)>,
    },
    InterfaceDecl {
        name: String,
        methods: Vec<InterfaceMethod>,
    },
    ImplInterface {
        interface_name: String,
        struct_name: String,
        methods: Vec<Statement>,
    },
    EnumDecl {
        name: String,
        variants: Vec<EnumVariant>,
    },
    LetDecl {
        name: String,
        type_annotation: Option<TypeAnnotation>,
        value: Expr,
    },
    Assignment {
        name: String,
        value: Expr,
    },
    IndexAssignment {
        target: Expr,
        index: Expr,
        value: Expr,
    },
    FieldAssignment {
        object: Box<Expr>,
        field: String,
        value: Expr,
    },
    FnDecl {
        name: String,
        params: Vec<(String, Option<TypeAnnotation>)>,
        return_type: Option<TypeAnnotation>,
        body: Vec<Statement>,
    },
    MethodDecl {
        struct_name: String,
        method_name: String,
        params: Vec<(String, Option<TypeAnnotation>)>,
        body: Vec<Statement>,
        return_type: Option<TypeAnnotation>,
    },
    Return(Expr),
    IfElse {
        condition: Expr,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
    },
    ForLoop {
        variable: String,
        iterable: Expr,
        body: Vec<Statement>,
    },
    Break,
    Continue,
    TryCatch {
        try_body: Vec<Statement>,
        catch_var: String,
        catch_body: Vec<Statement>,
    },
    Match {
        expr: Expr,
        arms: Vec<MatchArm>,
    },
    Print(Expr),
    ExprStatement(Expr),
}

/// 表達式。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    NullLit,
    Ident(String),
    StructInit {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    EnumVariant {
        enum_name: String,
        variant: String,
        fields: Option<Vec<(String, Expr)>>,
    },
    ArrayLit(Vec<Expr>),
    MapLit(Vec<(Expr, Expr)>),
    IndexAccess {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expr>,
    },
    FnCall {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Lambda {
        params: Vec<String>,
        body: Vec<Statement>,
    },
}

/// match arm。
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Statement>,
}

/// match pattern。
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
    EnumVariant {
        enum_name: String,
        variant: String,
        bindings: Option<Vec<String>>,
    },
    Wildcard,
}

/// 二元運算子。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

/// 一元運算子。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Neg,
    Not,
}

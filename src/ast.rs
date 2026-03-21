//! 抽象語法樹（AST）定義。
//!
//! parser 會把 token 流轉成 AST，
//! interpreter 與之後的 compiler 會基於這些節點工作。

/// 整份 tiny-lang 程式。
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
    /// 轉成人類可讀的型別名稱，用於錯誤訊息。
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

/// 陳述式節點。
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Import { path: String },
    StructDecl {
        name: String,
        fields: Vec<(String, Option<TypeAnnotation>)>,
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

/// 表達式節點。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
    StructInit {
        name: String,
        fields: Vec<(String, Expr)>,
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

/// match 使用的 pattern。
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
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

//! AST（抽象語法樹）定義。
//!
//! AST 是 parser 輸出的樹狀結構。
//! interpreter 不需要再面對原始字串，
//! 只要沿著 AST 節點逐步執行即可。

/// 一份程式就是一串 statement。
pub type Program = Vec<Statement>;

/// 陳述式。
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    LetDecl { name: String, value: Expr },
    Assignment { name: String, value: Expr },
    IndexAssignment { array: String, index: Expr, value: Expr },
    FnDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
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
    Print(Expr),
    ExprStatement(Expr),
}

/// 表達式。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
    ArrayLit(Vec<Expr>),
    IndexAccess {
        array: Box<Expr>,
        index: Box<Expr>,
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
        name: String,
        args: Vec<Expr>,
    },
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

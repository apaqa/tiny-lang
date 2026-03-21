//! 抽象語法樹（AST）定義。
//!
//! parser 會把 token 串整理成 AST，
//! interpreter 再依照這裡的結構執行程式。

/// 整份程式就是 statement 的列表。
pub type Program = Vec<Statement>;

/// 可執行的敘述節點。
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    LetDecl { name: String, value: Expr },
    Assignment { name: String, value: Expr },
    IndexAssignment { target: Expr, index: Expr, value: Expr },
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
    Print(Expr),
    ExprStatement(Expr),
}

/// 可計算出值的表達式節點。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
    ArrayLit(Vec<Expr>),
    MapLit(Vec<(Expr, Expr)>),
    IndexAccess {
        target: Box<Expr>,
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
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Lambda {
        params: Vec<String>,
        body: Vec<Statement>,
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

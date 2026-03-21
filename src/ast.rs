//! AST（抽象語法樹）定義。
//!
//! AST 是把程式從字串轉成「樹狀結構」後的表示法，
//! 後面的直譯器就可以直接沿著這棵樹執行。

/// 一份程式就是多個 statement。
pub type Program = Vec<Statement>;

/// 會改變程式狀態的語法單位。
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    LetDecl { name: String, value: Expr },
    Assignment { name: String, value: Expr },
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

/// 會產生值的語法單位。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
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

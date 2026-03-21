//! Token 定義。

/// 原始碼位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// 帶位置資訊的 token。
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// tiny-lang token 列舉。
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Let,
    Fn,
    Struct,
    Enum,
    New,
    Match,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Try,
    Catch,
    Import,
    Print,
    True,
    False,
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Assign,
    Arrow,
    FatArrow,
    And,
    Or,
    Not,
    Pipe,
    Colon,
    ColonColon,
    Dot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Eof,
}

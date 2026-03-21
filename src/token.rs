//! Token 與位置信息定義。
//!
//! Phase 2 開始，我們除了保留 token 種類，
//! 也讓 lexer 可以附帶每個 token 的行列位置，
//! 方便 parser 產生更好的錯誤訊息。

/// 原始碼中的一個位置。
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

/// 附帶位置資訊的 token。
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// tiny-lang 的所有 token。
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Let,
    Fn,
    Return,
    If,
    Else,
    While,
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
    And,
    Or,
    Not,
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

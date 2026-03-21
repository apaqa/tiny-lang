//! Token 定義。
//!
//! 這個檔案集中 tiny-lang 的詞彙單位，供 lexer 產生、
//! parser 消費，並讓錯誤訊息可以攜帶對應位置。

/// 原始碼中的行列位置。
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

/// 帶有位置資訊的 token。
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// tiny-lang 的所有 token 種類。
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Let,
    Fn,
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
    And,
    Or,
    Not,
    Pipe,
    Colon,
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

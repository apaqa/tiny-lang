//! Token 定義。
//!
//! Lexer 會把原始程式碼切成 token，
//! Parser 再根據 token 組出 AST。

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
    Comma,
    Semicolon,
    Eof,
}

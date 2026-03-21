//! 專案共用錯誤型別。
//!
//! 讓 lexer / parser / interpreter 都用同一個錯誤入口，
//! API 會比較乾淨，也比較容易測試。

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum TinyLangError {
    Lex(String),
    Parse(String),
    Runtime(String),
    Io(String),
}

impl Display for TinyLangError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TinyLangError::Lex(message) => write!(f, "Lexer error: {message}"),
            TinyLangError::Parse(message) => write!(f, "Parser error: {message}"),
            TinyLangError::Runtime(message) => write!(f, "Runtime error: {message}"),
            TinyLangError::Io(message) => write!(f, "IO error: {message}"),
        }
    }
}

impl std::error::Error for TinyLangError {}

impl From<std::io::Error> for TinyLangError {
    fn from(value: std::io::Error) -> Self {
        TinyLangError::Io(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, TinyLangError>;

//! 專案共用錯誤型別。
//!
//! Phase 2 把錯誤提升成統一結構：
//! - 錯誤種類
//! - 訊息
//! - 可選的行號 / 列號

use std::fmt::{Display, Formatter};

use crate::token::Span;

/// 錯誤分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Lex,
    Parse,
    Runtime,
    Io,
    /// 靜態型別檢查錯誤（在執行前由 TypeChecker 產生）
    TypeCheck,
}

/// tiny-lang 統一錯誤。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TinyLangError {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<Span>,
}

impl TinyLangError {
    pub fn lex(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: ErrorKind::Lex,
            message: message.into(),
            span: Some(span),
        }
    }

    pub fn parse(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: ErrorKind::Parse,
            message: message.into(),
            span: Some(span),
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: message.into(),
            span: None,
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Io,
            message: message.into(),
            span: None,
        }
    }

    /// 靜態型別檢查錯誤，可選附帶行列號。
    pub fn type_check(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::TypeCheck,
            message: message.into(),
            span: None,
        }
    }

    /// 帶行列號的靜態型別檢查錯誤。
    pub fn type_check_at(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: ErrorKind::TypeCheck,
            message: message.into(),
            span: Some(span),
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    fn kind_label(&self) -> &'static str {
        match self.kind {
            ErrorKind::Lex => "Lexer",
            ErrorKind::Parse => "Parser",
            ErrorKind::Runtime => "Runtime",
            ErrorKind::Io => "IO",
            ErrorKind::TypeCheck => "TypeError",
        }
    }
}

impl Display for TinyLangError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(span) = self.span {
            write!(
                f,
                "[line {}, col {}] {} error: {}",
                span.line,
                span.column,
                self.kind_label(),
                self.message
            )
        } else {
            write!(f, "{} error: {}", self.kind_label(), self.message)
        }
    }
}

impl std::error::Error for TinyLangError {}

impl From<std::io::Error> for TinyLangError {
    fn from(value: std::io::Error) -> Self {
        TinyLangError::io(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, TinyLangError>;

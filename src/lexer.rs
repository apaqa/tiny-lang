//! Lexer（詞法分析器）。
//!
//! 職責是把原始字串切成 token：
//! `let x = 10;`
//! ->
//! `Let Ident("x") Assign IntLit(10) Semicolon`

use crate::error::{Result, TinyLangError};
use crate::token::Token;

pub struct Lexer {
    chars: Vec<char>,
    position: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            position: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' | '\n' => {
                    self.advance();
                }
                '/' if self.peek_next() == Some('/') => self.skip_comment(),
                '0'..='9' => tokens.push(self.read_number()?),
                '"' => tokens.push(self.read_string()?),
                'a'..='z' | 'A'..='Z' | '_' => tokens.push(self.read_identifier_or_keyword()),
                '+' => {
                    self.advance();
                    tokens.push(Token::Plus);
                }
                '-' => {
                    self.advance();
                    tokens.push(Token::Minus);
                }
                '*' => {
                    self.advance();
                    tokens.push(Token::Star);
                }
                '/' => {
                    self.advance();
                    tokens.push(Token::Slash);
                }
                '%' => {
                    self.advance();
                    tokens.push(Token::Percent);
                }
                '=' => {
                    self.advance();
                    if self.match_char('=') {
                        tokens.push(Token::Eq);
                    } else {
                        tokens.push(Token::Assign);
                    }
                }
                '!' => {
                    self.advance();
                    if self.match_char('=') {
                        tokens.push(Token::Ne);
                    } else {
                        tokens.push(Token::Not);
                    }
                }
                '<' => {
                    self.advance();
                    if self.match_char('=') {
                        tokens.push(Token::Le);
                    } else {
                        tokens.push(Token::Lt);
                    }
                }
                '>' => {
                    self.advance();
                    if self.match_char('=') {
                        tokens.push(Token::Ge);
                    } else {
                        tokens.push(Token::Gt);
                    }
                }
                '&' => {
                    self.advance();
                    if self.match_char('&') {
                        tokens.push(Token::And);
                    } else {
                        return Err(TinyLangError::Lex("單一 '&' 不合法，請使用 &&".into()));
                    }
                }
                '|' => {
                    self.advance();
                    if self.match_char('|') {
                        tokens.push(Token::Or);
                    } else {
                        return Err(TinyLangError::Lex("單一 '|' 不合法，請使用 ||".into()));
                    }
                }
                '(' => {
                    self.advance();
                    tokens.push(Token::LParen);
                }
                ')' => {
                    self.advance();
                    tokens.push(Token::RParen);
                }
                '{' => {
                    self.advance();
                    tokens.push(Token::LBrace);
                }
                '}' => {
                    self.advance();
                    tokens.push(Token::RBrace);
                }
                ',' => {
                    self.advance();
                    tokens.push(Token::Comma);
                }
                ';' => {
                    self.advance();
                    tokens.push(Token::Semicolon);
                }
                _ => return Err(TinyLangError::Lex(format!("無法辨識的字元: '{ch}'"))),
            }
        }

        tokens.push(Token::Eof);
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.position + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.position += 1;
        }
        ch
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn skip_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    fn read_number(&mut self) -> Result<Token> {
        let start = self.position;
        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }

        let number_str: String = self.chars[start..self.position].iter().collect();
        let value = number_str.parse::<i64>().map_err(|err| {
            TinyLangError::Lex(format!("整數解析失敗 '{number_str}': {err}"))
        })?;
        Ok(Token::IntLit(value))
    }

    fn read_string(&mut self) -> Result<Token> {
        self.advance();
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.advance();
                    return Ok(Token::StringLit(value));
                }
                '\\' => {
                    self.advance();
                    let escaped = self
                        .advance()
                        .ok_or_else(|| TinyLangError::Lex("字串跳脫字元不完整".into()))?;
                    let actual = match escaped {
                        'n' => '\n',
                        't' => '\t',
                        '"' => '"',
                        '\\' => '\\',
                        other => other,
                    };
                    value.push(actual);
                }
                _ => {
                    value.push(ch);
                    self.advance();
                }
            }
        }

        Err(TinyLangError::Lex("字串缺少結尾雙引號".into()))
    }

    fn read_identifier_or_keyword(&mut self) -> Token {
        let start = self.position;
        while matches!(self.peek(), Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_')) {
            self.advance();
        }

        let ident: String = self.chars[start..self.position].iter().collect();
        match ident.as_str() {
            "let" => Token::Let,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "print" => Token::Print,
            "true" => Token::BoolLit(true),
            "false" => Token::BoolLit(false),
            _ => Token::Ident(ident),
        }
    }
}

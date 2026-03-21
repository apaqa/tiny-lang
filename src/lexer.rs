//! Lexer 實作。
//!
//! 這一層把原始碼切成 token，並保留行列資訊，
//! 讓 parser 與錯誤訊息可以精準指出位置。

use crate::error::{Result, TinyLangError};
use crate::token::{Span, SpannedToken, Token};

pub struct Lexer {
    chars: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    /// 將輸入切成不帶位置資訊的 token，供舊測試或簡化流程使用。
    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let spanned = self.tokenize_with_spans()?;
        Ok(spanned.into_iter().map(|item| item.token).collect())
    }

    /// 將輸入切成帶位置資訊的 token 串。
    pub fn tokenize_with_spans(&mut self) -> Result<Vec<SpannedToken>> {
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
                '+' => tokens.push(self.single_char(Token::Plus)),
                '-' => tokens.push(self.single_char(Token::Minus)),
                '*' => tokens.push(self.single_char(Token::Star)),
                '/' => tokens.push(self.single_char(Token::Slash)),
                '%' => tokens.push(self.single_char(Token::Percent)),
                '(' => tokens.push(self.single_char(Token::LParen)),
                ')' => tokens.push(self.single_char(Token::RParen)),
                '{' => tokens.push(self.single_char(Token::LBrace)),
                '}' => tokens.push(self.single_char(Token::RBrace)),
                '[' => tokens.push(self.single_char(Token::LBracket)),
                ']' => tokens.push(self.single_char(Token::RBracket)),
                ',' => tokens.push(self.single_char(Token::Comma)),
                ';' => tokens.push(self.single_char(Token::Semicolon)),
                ':' => tokens.push(self.single_char(Token::Colon)),
                '=' => tokens.push(self.read_two_char(Token::Assign, '=', Token::Eq)),
                '!' => tokens.push(self.read_two_char(Token::Not, '=', Token::Ne)),
                '<' => tokens.push(self.read_two_char(Token::Lt, '=', Token::Le)),
                '>' => tokens.push(self.read_two_char(Token::Gt, '=', Token::Ge)),
                '&' => {
                    let span = self.current_span();
                    self.advance();
                    if self.match_char('&') {
                        tokens.push(SpannedToken {
                            token: Token::And,
                            span,
                        });
                    } else {
                        return Err(TinyLangError::lex("single '&' is not valid, use &&", span));
                    }
                }
                '|' => {
                    let span = self.current_span();
                    self.advance();
                    let token = if self.match_char('|') {
                        Token::Or
                    } else {
                        Token::Pipe
                    };
                    tokens.push(SpannedToken { token, span });
                }
                _ => {
                    return Err(TinyLangError::lex(
                        format!("unrecognized character '{ch}'"),
                        self.current_span(),
                    ));
                }
            }
        }

        tokens.push(SpannedToken {
            token: Token::Eof,
            span: self.current_span(),
        });
        Ok(tokens)
    }

    fn single_char(&mut self, token: Token) -> SpannedToken {
        let span = self.current_span();
        self.advance();
        SpannedToken { token, span }
    }

    fn read_two_char(&mut self, one: Token, expected: char, two: Token) -> SpannedToken {
        let span = self.current_span();
        self.advance();
        let token = if self.match_char(expected) { two } else { one };
        SpannedToken { token, span }
    }

    fn current_span(&self) -> Span {
        Span::new(self.line, self.column)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.position + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.position += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
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

    fn read_number(&mut self) -> Result<SpannedToken> {
        let span = self.current_span();
        let start = self.position;

        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }

        let number_str: String = self.chars[start..self.position].iter().collect();
        let value = number_str
            .parse::<i64>()
            .map_err(|err| TinyLangError::lex(format!("invalid integer '{number_str}': {err}"), span))?;

        Ok(SpannedToken {
            token: Token::IntLit(value),
            span,
        })
    }

    fn read_string(&mut self) -> Result<SpannedToken> {
        let span = self.current_span();
        self.advance();
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.advance();
                    return Ok(SpannedToken {
                        token: Token::StringLit(value),
                        span,
                    });
                }
                '\\' => {
                    self.advance();
                    let escaped = self
                        .advance()
                        .ok_or_else(|| TinyLangError::lex("unterminated escape sequence", span))?;
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

        Err(TinyLangError::lex("unterminated string literal", span))
    }

    fn read_identifier_or_keyword(&mut self) -> SpannedToken {
        let span = self.current_span();
        let start = self.position;

        while matches!(self.peek(), Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_')) {
            self.advance();
        }

        let ident: String = self.chars[start..self.position].iter().collect();
        let token = match ident.as_str() {
            "let" => Token::Let,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "try" => Token::Try,
            "catch" => Token::Catch,
            "print" => Token::Print,
            "true" => Token::BoolLit(true),
            "false" => Token::BoolLit(false),
            _ => Token::Ident(ident),
        };

        SpannedToken { token, span }
    }
}

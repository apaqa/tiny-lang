//! Parser（語法分析器）。
//!
//! 這裡用遞迴下降（recursive descent）方式，
//! 按照運算子優先級把 token 組成 AST。

use crate::ast::{BinaryOperator, Expr, Program, Statement, UnaryOperator};
use crate::error::{Result, TinyLangError};
use crate::token::Token;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, position: 0 }
    }

    pub fn parse(&mut self) -> Result<Program> {
        let mut program = Vec::new();
        while !self.is_at_end() {
            program.push(self.parse_statement()?);
        }
        Ok(program)
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        match self.peek() {
            Token::Let => self.parse_let_decl(),
            Token::Fn => self.parse_fn_decl(),
            Token::Return => self.parse_return_stmt(),
            Token::If => self.parse_if_else_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::Print => self.parse_print_stmt(),
            Token::Ident(_) if self.peek_next() == Some(&Token::Assign) => self.parse_assignment(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Let)?;
        let name = self.consume_ident()?;
        self.expect_token(Token::Assign)?;
        let value = self.parse_expression()?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::LetDecl { name, value })
    }

    fn parse_assignment(&mut self) -> Result<Statement> {
        let name = self.consume_ident()?;
        self.expect_token(Token::Assign)?;
        let value = self.parse_expression()?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Assignment { name, value })
    }

    fn parse_fn_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Fn)?;
        let name = self.consume_ident()?;
        self.expect_token(Token::LParen)?;

        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                params.push(self.consume_ident()?);
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }

        self.expect_token(Token::RParen)?;
        let body = self.parse_block()?;
        Ok(Statement::FnDecl { name, params, body })
    }

    fn parse_return_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Return)?;
        let expr = self.parse_expression()?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Return(expr))
    }

    fn parse_if_else_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::If)?;
        let condition = self.parse_expression()?;
        let then_body = self.parse_block()?;
        let else_body = if self.match_token(&Token::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Statement::IfElse {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::While)?;
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::While { condition, body })
    }

    fn parse_print_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Print)?;
        self.expect_token(Token::LParen)?;
        let expr = self.parse_expression()?;
        self.expect_token(Token::RParen)?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Print(expr))
    }

    fn parse_expr_stmt(&mut self) -> Result<Statement> {
        let expr = self.parse_expression()?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::ExprStatement(expr))
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>> {
        self.expect_token(Token::LBrace)?;
        let mut statements = Vec::new();

        while !self.check(&Token::RBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        self.expect_token(Token::RBrace)?;
        Ok(statements)
    }

    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Expr> {
        let mut expr = self.parse_logical_and()?;
        while self.match_token(&Token::Or) {
            let right = self.parse_logical_and()?;
            expr = Expr::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expr> {
        let mut expr = self.parse_comparison()?;
        while self.match_token(&Token::And) {
            let right = self.parse_comparison()?;
            expr = Expr::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut expr = self.parse_term()?;

        loop {
            let op = match self.peek() {
                Token::Eq => BinaryOperator::Eq,
                Token::Ne => BinaryOperator::Ne,
                Token::Lt => BinaryOperator::Lt,
                Token::Gt => BinaryOperator::Gt,
                Token::Le => BinaryOperator::Le,
                Token::Ge => BinaryOperator::Ge,
                _ => break,
            };
            self.advance();

            let right = self.parse_term()?;
            expr = Expr::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr> {
        let mut expr = self.parse_factor()?;

        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.advance();

            let right = self.parse_factor()?;
            expr = Expr::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr> {
        let mut expr = self.parse_unary()?;

        loop {
            let op = match self.peek() {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                Token::Percent => BinaryOperator::Mod,
                _ => break,
            };
            self.advance();

            let right = self.parse_unary()?;
            expr = Expr::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOperator::Neg,
                    operand: Box::new(operand),
                })
            }
            Token::Not => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOperator::Not,
                    operand: Box::new(operand),
                })
            }
            _ => self.parse_call(),
        }
    }

    fn parse_call(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            if !self.match_token(&Token::LParen) {
                break;
            }

            let name = match expr {
                Expr::Ident(name) => name,
                _ => {
                    return Err(TinyLangError::Parse(
                        "只有識別字後面可以接函式呼叫".into(),
                    ))
                }
            };

            let mut args = Vec::new();
            if !self.check(&Token::RParen) {
                loop {
                    args.push(self.parse_expression()?);
                    if !self.match_token(&Token::Comma) {
                        break;
                    }
                }
            }
            self.expect_token(Token::RParen)?;

            expr = Expr::FnCall { name, args };
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        let token = self.advance().clone();
        match token {
            Token::IntLit(value) => Ok(Expr::IntLit(value)),
            Token::StringLit(value) => Ok(Expr::StringLit(value)),
            Token::BoolLit(value) => Ok(Expr::BoolLit(value)),
            Token::True => Ok(Expr::BoolLit(true)),
            Token::False => Ok(Expr::BoolLit(false)),
            Token::Ident(name) => Ok(Expr::Ident(name)),
            Token::LParen => {
                let expr = self.parse_expression()?;
                self.expect_token(Token::RParen)?;
                Ok(expr)
            }
            other => Err(TinyLangError::Parse(format!(
                "不預期的 token，無法作為 expression: {other:?}"
            ))),
        }
    }

    fn consume_ident(&mut self) -> Result<String> {
        match self.advance().clone() {
            Token::Ident(name) => Ok(name),
            other => Err(TinyLangError::Parse(format!(
                "預期識別字，實際遇到 {other:?}"
            ))),
        }
    }

    fn expect_token(&mut self, expected: Token) -> Result<()> {
        if self.check(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(TinyLangError::Parse(format!(
                "預期 token {expected:?}，實際遇到 {:?}",
                self.peek()
            )))
        }
    }

    fn match_token(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, expected: &Token) -> bool {
        self.peek() == expected
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::Eof)
    }

    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1)
    }

    fn advance(&mut self) -> &Token {
        let index = self.position;
        if !self.is_at_end() {
            self.position += 1;
        }
        self.tokens.get(index).unwrap_or(&Token::Eof)
    }
}

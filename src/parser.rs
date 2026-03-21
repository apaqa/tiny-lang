//! Parser 實作。
//!
//! 這裡使用遞迴下降 parser，將 token 串轉成 AST。
//! Phase 3 追加了 for、map、lambda、try/catch 與 break/continue。

use crate::ast::{BinaryOperator, Expr, Program, Statement, UnaryOperator};
use crate::error::{Result, TinyLangError};
use crate::token::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
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
        match &self.peek().token {
            Token::Let => self.parse_let_decl(),
            Token::Fn if matches!(self.peek_next_token(), Some(Token::Ident(_))) => self.parse_fn_decl(),
            Token::Return => self.parse_return_stmt(),
            Token::If => self.parse_if_else_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Break => self.parse_break_stmt(),
            Token::Continue => self.parse_continue_stmt(),
            Token::Try => self.parse_try_catch_stmt(),
            Token::Print => self.parse_print_stmt(),
            Token::Ident(_) if self.peek_next_token() == Some(&Token::Assign) => self.parse_assignment(),
            Token::Ident(_) if self.looks_like_index_assignment() => self.parse_index_assignment(),
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

    fn parse_index_assignment(&mut self) -> Result<Statement> {
        let mut target = Expr::Ident(self.consume_ident()?);
        while self.match_token(&Token::LBracket) {
            let index = self.parse_expression()?;
            self.expect_token(Token::RBracket)?;
            if self.check(&Token::Assign) {
                self.expect_token(Token::Assign)?;
                let value = self.parse_expression()?;
                self.expect_token(Token::Semicolon)?;
                return Ok(Statement::IndexAssignment { target, index, value });
            }

            target = Expr::IndexAccess {
                target: Box::new(target),
                index: Box::new(index),
            };
        }

        Err(TinyLangError::parse(
            "expected '=' after index assignment target",
            self.peek().span,
        ))
    }

    fn parse_fn_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Fn)?;
        let name = self.consume_ident()?;
        let params = self.parse_parameter_list()?;
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

    fn parse_for_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::For)?;
        let variable = self.consume_ident()?;
        self.expect_token(Token::In)?;
        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::ForLoop {
            variable,
            iterable,
            body,
        })
    }

    fn parse_break_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Break)?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Break)
    }

    fn parse_continue_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Continue)?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Continue)
    }

    fn parse_try_catch_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Try)?;
        let try_body = self.parse_block()?;
        self.expect_token(Token::Catch)?;
        let catch_var = self.consume_ident()?;
        let catch_body = self.parse_block()?;
        Ok(Statement::TryCatch {
            try_body,
            catch_var,
            catch_body,
        })
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

    fn parse_parameter_list(&mut self) -> Result<Vec<String>> {
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
        Ok(params)
    }

    fn parse_lambda_pipe_params(&mut self) -> Result<Vec<String>> {
        self.expect_token(Token::Pipe)?;
        let mut params = Vec::new();
        if !self.check(&Token::Pipe) {
            loop {
                params.push(self.consume_ident()?);
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(Token::Pipe)?;
        Ok(params)
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
            let op = match self.peek().token {
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
            let op = match self.peek().token {
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
            let op = match self.peek().token {
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
        match self.peek().token {
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
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&Token::LParen) {
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
                expr = Expr::FnCall {
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }

            if self.match_token(&Token::LBracket) {
                let index = self.parse_expression()?;
                self.expect_token(Token::RBracket)?;
                expr = Expr::IndexAccess {
                    target: Box::new(expr),
                    index: Box::new(index),
                };
                continue;
            }

            break;
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match self.peek().token.clone() {
            Token::Fn => self.parse_fn_lambda(),
            Token::Pipe => self.parse_pipe_lambda(),
            _ => {
                let token = self.advance().clone();
                match token.token {
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
                    Token::LBracket => self.parse_array_literal(),
                    Token::LBrace => self.parse_map_literal(),
                    other => Err(TinyLangError::parse(
                        format!("unexpected token in expression: {other:?}"),
                        token.span,
                    )),
                }
            }
        }
    }

    fn parse_array_literal(&mut self) -> Result<Expr> {
        let mut items = Vec::new();
        if !self.check(&Token::RBracket) {
            loop {
                items.push(self.parse_expression()?);
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(Token::RBracket)?;
        Ok(Expr::ArrayLit(items))
    }

    fn parse_map_literal(&mut self) -> Result<Expr> {
        let mut items = Vec::new();
        if !self.check(&Token::RBrace) {
            loop {
                let key = self.parse_expression()?;
                self.expect_token(Token::Colon)?;
                let value = self.parse_expression()?;
                items.push((key, value));
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(Token::RBrace)?;
        Ok(Expr::MapLit(items))
    }

    fn parse_fn_lambda(&mut self) -> Result<Expr> {
        self.expect_token(Token::Fn)?;
        let params = self.parse_parameter_list()?;
        let body = self.parse_block()?;
        Ok(Expr::Lambda { params, body })
    }

    fn parse_pipe_lambda(&mut self) -> Result<Expr> {
        let params = self.parse_lambda_pipe_params()?;
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            vec![Statement::Return(self.parse_expression()?)]
        };
        Ok(Expr::Lambda { params, body })
    }

    fn looks_like_index_assignment(&self) -> bool {
        if !matches!(self.peek().token, Token::Ident(_)) || self.peek_next_token() != Some(&Token::LBracket) {
            return false;
        }

        let mut cursor = self.position + 1;
        let mut depth = 0_i32;

        while let Some(item) = self.tokens.get(cursor) {
            match item.token {
                Token::LBracket => depth += 1,
                Token::RBracket => {
                    depth -= 1;
                    if depth == 0
                        && matches!(
                            self.tokens.get(cursor + 1).map(|token| &token.token),
                            Some(Token::Assign)
                        )
                    {
                        return true;
                    }
                }
                Token::Semicolon | Token::Eof => return false,
                _ => {}
            }
            cursor += 1;
        }

        false
    }

    fn consume_ident(&mut self) -> Result<String> {
        let token = self.advance().clone();
        match token.token {
            Token::Ident(name) => Ok(name),
            other => Err(TinyLangError::parse(
                format!("expected identifier, got {other:?}"),
                token.span,
            )),
        }
    }

    fn expect_token(&mut self, expected: Token) -> Result<()> {
        if self.check(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(TinyLangError::parse(
                format!("expected token {expected:?}, got {:?}", self.peek().token),
                self.peek().span,
            ))
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
        self.peek().token == *expected
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().token, Token::Eof)
    }

    fn peek(&self) -> &SpannedToken {
        self.tokens.get(self.position).unwrap_or_else(|| self.tokens.last().expect("token stream must not be empty"))
    }

    fn peek_next_token(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1).map(|item| &item.token)
    }

    fn advance(&mut self) -> &SpannedToken {
        let index = self.position;
        if !self.is_at_end() {
            self.position += 1;
        }
        self.tokens.get(index).unwrap_or_else(|| self.tokens.last().expect("token stream must not be empty"))
    }
}

//! Parser。

use crate::ast::{
    BinaryOperator, EnumVariant, Expr, InterfaceMethod, MatchArm, Pattern, Program, Statement,
    TypeAnnotation, UnaryOperator,
};
use crate::error::{Result, TinyLangError};
use crate::token::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    position: usize,
    allow_struct_init: bool,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self {
            tokens,
            position: 0,
            allow_struct_init: true,
        }
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
            Token::Import => self.parse_import_stmt(),
            Token::Struct => self.parse_struct_decl(),
            Token::Interface => self.parse_interface_decl(),
            Token::Impl => self.parse_impl_interface_decl(),
            Token::Enum => self.parse_enum_decl(),
            Token::Let => self.parse_let_decl(),
            Token::Fn => self.parse_fn_or_method_decl(),
            Token::Return => self.parse_return_stmt(),
            Token::If => self.parse_if_else_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Break => self.parse_break_stmt(),
            Token::Continue => self.parse_continue_stmt(),
            Token::Try => self.parse_try_catch_stmt(),
            Token::Match => self.parse_match_stmt(),
            Token::Print => self.parse_print_stmt(),
            _ => self.parse_expr_or_assignment_stmt(),
        }
    }

    fn parse_import_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Import)?;
        let path_token = self.advance().clone();
        let Token::StringLit(path) = path_token.token else {
            return Err(TinyLangError::parse("import expects a string path", path_token.span));
        };
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Import { path })
    }

    fn parse_struct_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Struct)?;
        let name = self.consume_ident()?;
        self.expect_token(Token::LBrace)?;
        let mut fields = Vec::new();
        if !self.check(&Token::RBrace) {
            loop {
                let field_name = self.consume_ident()?;
                let annotation = if self.match_token(&Token::Colon) {
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };
                fields.push((field_name, annotation));
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(Token::RBrace)?;
        Ok(Statement::StructDecl { name, fields })
    }

    fn parse_interface_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Interface)?;
        let name = self.consume_ident()?;
        self.expect_token(Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            self.expect_token(Token::Fn)?;
            let method_name = self.consume_ident()?;
            let params = self.parse_typed_parameter_list()?;
            let return_type = if self.match_token(&Token::Arrow) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            self.expect_token(Token::Semicolon)?;
            methods.push(InterfaceMethod {
                name: method_name,
                params,
                return_type,
            });
        }
        self.expect_token(Token::RBrace)?;
        Ok(Statement::InterfaceDecl { name, methods })
    }

    fn parse_impl_interface_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Impl)?;
        let interface_name = self.consume_ident()?;
        self.expect_token(Token::For)?;
        let struct_name = self.consume_ident()?;
        self.expect_token(Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            self.expect_token(Token::Fn)?;
            let name = self.consume_ident()?;
            let params = self.parse_typed_parameter_list()?;
            let return_type = if self.match_token(&Token::Arrow) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            let body = self.parse_block()?;
            methods.push(Statement::FnDecl {
                name,
                type_params: Vec::new(),
                params,
                return_type,
                body,
            });
        }
        self.expect_token(Token::RBrace)?;
        Ok(Statement::ImplInterface {
            interface_name,
            struct_name,
            methods,
        })
    }

    fn parse_enum_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Enum)?;
        let name = self.consume_ident()?;
        self.expect_token(Token::LBrace)?;
        let mut variants = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            let variant_name = self.consume_ident()?;
            let fields = if self.match_token(&Token::LBrace) {
                let mut fields = Vec::new();
                if !self.check(&Token::RBrace) {
                    loop {
                        let field_name = self.consume_ident()?;
                        let annotation = if self.match_token(&Token::Colon) {
                            Some(self.parse_type_annotation()?)
                        } else {
                            None
                        };
                        fields.push((field_name, annotation));
                        if !self.match_token(&Token::Comma) {
                            break;
                        }
                    }
                }
                self.expect_token(Token::RBrace)?;
                Some(fields)
            } else {
                None
            };
            variants.push(EnumVariant {
                name: variant_name,
                fields,
            });
            self.match_token(&Token::Comma);
        }
        self.expect_token(Token::RBrace)?;
        Ok(Statement::EnumDecl { name, variants })
    }

    fn parse_let_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Let)?;
        let name = self.consume_ident()?;
        let type_annotation = if self.match_token(&Token::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };
        self.expect_token(Token::Assign)?;
        let value = self.parse_expression()?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::LetDecl {
            name,
            type_annotation,
            value,
        })
    }

    fn parse_fn_or_method_decl(&mut self) -> Result<Statement> {
        self.expect_token(Token::Fn)?;
        let first_name = self.consume_ident()?;
        let type_params = self.parse_optional_type_params()?;

        if self.match_token(&Token::Dot) {
            let method_name = self.consume_ident()?;
            let params = self.parse_typed_parameter_list()?;
            let return_type = if self.match_token(&Token::Arrow) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            let body = self.parse_block()?;
            return Ok(Statement::MethodDecl {
                struct_name: first_name,
                method_name,
                params,
                body,
                return_type,
            });
        }

        let params = self.parse_typed_parameter_list()?;
        let return_type = if self.match_token(&Token::Arrow) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Statement::FnDecl {
            name: first_name,
            type_params,
            params,
            return_type,
            body,
        })
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

    fn parse_match_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Match)?;
        let previous = self.allow_struct_init;
        self.allow_struct_init = false;
        let expr = self.parse_expression()?;
        self.allow_struct_init = previous;
        self.expect_token(Token::LBrace)?;
        let mut arms = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            let pattern = self.parse_pattern()?;
            self.expect_token(Token::FatArrow)?;
            let body = self.parse_block()?;
            arms.push(MatchArm { pattern, body });
        }
        self.expect_token(Token::RBrace)?;
        Ok(Statement::Match { expr, arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        let token = self.advance().clone();
        match token.token {
            Token::IntLit(value) => Ok(Pattern::IntLit(value)),
            Token::StringLit(value) => Ok(Pattern::StringLit(value)),
            Token::BoolLit(value) => Ok(Pattern::BoolLit(value)),
            Token::True => Ok(Pattern::BoolLit(true)),
            Token::False => Ok(Pattern::BoolLit(false)),
            Token::Ident(name) if name == "_" => Ok(Pattern::Wildcard),
            Token::Ident(enum_name) if self.match_token(&Token::ColonColon) => {
                let variant = self.consume_ident()?;
                // 支援兩種 binding 語法：{ field1, field2 } 或 (field1, field2)
                let bindings = if self.match_token(&Token::LBrace) {
                    let mut fields = Vec::new();
                    if !self.check(&Token::RBrace) {
                        loop {
                            fields.push(self.consume_ident()?);
                            if !self.match_token(&Token::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect_token(Token::RBrace)?;
                    Some(fields)
                } else if self.match_token(&Token::LParen) {
                    // 括號語法：Result::Ok(binding) 用於捕獲 variant 欄位
                    let mut fields = Vec::new();
                    if !self.check(&Token::RParen) {
                        loop {
                            fields.push(self.consume_ident()?);
                            if !self.match_token(&Token::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect_token(Token::RParen)?;
                    Some(fields)
                } else {
                    None
                };
                Ok(Pattern::EnumVariant {
                    enum_name,
                    variant,
                    bindings,
                })
            }
            Token::Ident(name) => Ok(Pattern::Ident(name)),
            other => Err(TinyLangError::parse(
                format!("unexpected token in match pattern: {other:?}"),
                token.span,
            )),
        }
    }

    fn parse_print_stmt(&mut self) -> Result<Statement> {
        self.expect_token(Token::Print)?;
        self.expect_token(Token::LParen)?;
        let expr = self.parse_expression()?;
        self.expect_token(Token::RParen)?;
        self.expect_token(Token::Semicolon)?;
        Ok(Statement::Print(expr))
    }

    fn parse_expr_or_assignment_stmt(&mut self) -> Result<Statement> {
        let expr = self.parse_expression()?;
        if self.match_token(&Token::Assign) {
            let value = self.parse_expression()?;
            self.expect_token(Token::Semicolon)?;
            return match expr {
                Expr::Ident(name) => Ok(Statement::Assignment { name, value }),
                Expr::IndexAccess { target, index } => Ok(Statement::IndexAssignment {
                    target: *target,
                    index: *index,
                    value,
                }),
                Expr::FieldAccess { object, field } => Ok(Statement::FieldAssignment { object, field, value }),
                _ => Err(TinyLangError::parse(
                    "invalid assignment target",
                    self.peek_previous().span,
                )),
            };
        }

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

    fn parse_typed_parameter_list(&mut self) -> Result<Vec<(String, Option<TypeAnnotation>)>> {
        self.expect_token(Token::LParen)?;
        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                let name = self.consume_ident()?;
                let annotation = if self.match_token(&Token::Colon) {
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };
                params.push((name, annotation));
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(Token::RParen)?;
        Ok(params)
    }

    fn parse_optional_type_params(&mut self) -> Result<Vec<String>> {
        if !self.match_token(&Token::Lt) {
            return Ok(Vec::new());
        }

        let mut type_params = Vec::new();
        loop {
            type_params.push(self.consume_ident()?);
            if !self.match_token(&Token::Comma) {
                break;
            }
        }
        self.expect_token(Token::Gt)?;
        Ok(type_params)
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

    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation> {
        let token = self.advance().clone();
        match token.token {
            Token::Ident(name) => match name.as_str() {
                "int" => Ok(TypeAnnotation::Int),
                "str" => Ok(TypeAnnotation::Str),
                "bool" => Ok(TypeAnnotation::Bool),
                "any" => Ok(TypeAnnotation::Any),
                _ => {
                    if self.match_token(&Token::Lt) {
                        let mut type_params = Vec::new();
                        loop {
                            type_params.push(self.parse_type_annotation()?);
                            if !self.match_token(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect_token(Token::Gt)?;
                        Ok(TypeAnnotation::Generic { name, type_params })
                    } else {
                        Ok(TypeAnnotation::Named(name))
                    }
                }
            },
            Token::LBracket => {
                let inner = self.parse_type_annotation()?;
                self.expect_token(Token::RBracket)?;
                Ok(TypeAnnotation::ArrayOf(Box::new(inner)))
            }
            Token::LBrace => {
                let inner = self.parse_type_annotation()?;
                self.expect_token(Token::RBrace)?;
                Ok(TypeAnnotation::MapOf(Box::new(inner)))
            }
            other => Err(TinyLangError::parse(
                format!("unexpected token in type annotation: {other:?}"),
                token.span,
            )),
        }
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

            if self.match_token(&Token::Dot) {
                let field = self.consume_ident()?;
                expr = Expr::FieldAccess {
                    object: Box::new(expr),
                    field,
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
            // `||` 被 lexer 解析為 Token::Or，這裡視為空參數 lambda
            Token::Or => self.parse_empty_lambda(),
            Token::New => self.parse_new_struct_init(),
            _ => {
                let token = self.advance().clone();
                match token.token {
                    Token::IntLit(value) => Ok(Expr::IntLit(value)),
                    Token::StringLit(value) => Ok(Expr::StringLit(value)),
                    Token::BoolLit(value) => Ok(Expr::BoolLit(value)),
                    Token::True => Ok(Expr::BoolLit(true)),
                    Token::False => Ok(Expr::BoolLit(false)),
                    Token::Null => Ok(Expr::NullLit),
                    Token::Ident(name) => {
                        if self.match_token(&Token::ColonColon) {
                            return self.parse_enum_variant_after_name(name);
                        }
                        if self.allow_struct_init && self.check(&Token::LBrace) {
                            self.parse_struct_init_after_name(name)
                        } else {
                            Ok(Expr::Ident(name))
                        }
                    }
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

    fn parse_enum_variant_after_name(&mut self, enum_name: String) -> Result<Expr> {
        let variant = self.consume_ident()?;
        let fields = if self.match_token(&Token::LBrace) {
            let mut fields = Vec::new();
            if !self.check(&Token::RBrace) {
                loop {
                    let field_name = self.consume_ident()?;
                    self.expect_token(Token::Colon)?;
                    let value = self.parse_expression()?;
                    fields.push((field_name, value));
                    if !self.match_token(&Token::Comma) {
                        break;
                    }
                }
            }
            self.expect_token(Token::RBrace)?;
            Some(fields)
        } else if self.match_token(&Token::LParen) {
            // 中文註解：tuple-like enum variant 以 0、1、2... 當作內部欄位名稱。
            let mut fields = Vec::new();
            let mut index = 0;
            if !self.check(&Token::RParen) {
                loop {
                    let value = self.parse_expression()?;
                    fields.push((index.to_string(), value));
                    index += 1;
                    if !self.match_token(&Token::Comma) {
                        break;
                    }
                }
            }
            self.expect_token(Token::RParen)?;
            Some(fields)
        } else {
            None
        };
        Ok(Expr::EnumVariant {
            enum_name,
            variant,
            fields,
        })
    }

    fn parse_new_struct_init(&mut self) -> Result<Expr> {
        self.expect_token(Token::New)?;
        let name = self.consume_ident()?;
        self.parse_struct_init_after_name(name)
    }

    fn parse_struct_init_after_name(&mut self, name: String) -> Result<Expr> {
        self.expect_token(Token::LBrace)?;
        let mut fields = Vec::new();
        if !self.check(&Token::RBrace) {
            loop {
                let field_name = self.consume_ident()?;
                self.expect_token(Token::Colon)?;
                let value = self.parse_expression()?;
                fields.push((field_name, value));
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(Token::RBrace)?;
        Ok(Expr::StructInit { name, fields })
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

    /// 解析 `|| { ... }` 空參數 lambda（`||` 被 lexer 合併為 Token::Or）
    fn parse_empty_lambda(&mut self) -> Result<Expr> {
        // 消費 Token::Or（即 `||`）
        self.advance();
        let body = if self.check(&Token::LBrace) {
            self.parse_block()?
        } else {
            vec![Statement::Return(self.parse_expression()?)]
        };
        Ok(Expr::Lambda { params: vec![], body })
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
        self.tokens
            .get(self.position)
            .unwrap_or_else(|| self.tokens.last().expect("token stream must not be empty"))
    }

    fn peek_previous(&self) -> &SpannedToken {
        let index = self.position.saturating_sub(1);
        self.tokens
            .get(index)
            .unwrap_or_else(|| self.tokens.last().expect("token stream must not be empty"))
    }

    fn advance(&mut self) -> &SpannedToken {
        let index = self.position;
        if !self.is_at_end() {
            self.position += 1;
        }
        self.tokens
            .get(index)
            .unwrap_or_else(|| self.tokens.last().expect("token stream must not be empty"))
    }
}

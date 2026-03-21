//! Bytecode compiler 的基礎骨架。
//!
//! 這個階段先提供可擴充的指令集與編譯入口，
//! 之後可以再接 VM 與更多 lowering 規則。

use crate::ast::{BinaryOperator, Expr, Program, Statement, UnaryOperator};
use crate::error::{Result, TinyLangError};

/// 常數池中的值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constant {
    Int(i64),
    Str(String),
    Bool(bool),
    Symbol(String),
}

/// 基礎 bytecode 指令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    LoadConst(usize),
    LoadName(usize),
    StoreName(usize),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Not,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Print,
    Pop,
}

/// 編譯輸出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BytecodeChunk {
    pub constants: Vec<Constant>,
    pub instructions: Vec<Instruction>,
}

/// Phase 5 的 compiler 先涵蓋基礎節點，其他節點先回傳明確錯誤。
#[derive(Debug, Default)]
pub struct Compiler {
    chunk: BytecodeChunk,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compile_program(mut self, program: &Program) -> Result<BytecodeChunk> {
        for statement in program {
            self.compile_statement(statement)?;
        }
        Ok(self.chunk)
    }

    fn compile_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::LetDecl { name, value, .. } => {
                self.compile_expr(value)?;
                let slot = self.push_constant(Constant::Symbol(name.clone()));
                self.chunk.instructions.push(Instruction::StoreName(slot));
            }
            Statement::Assignment { name, value } => {
                self.compile_expr(value)?;
                let slot = self.push_constant(Constant::Symbol(name.clone()));
                self.chunk.instructions.push(Instruction::StoreName(slot));
            }
            Statement::Print(expr) => {
                self.compile_expr(expr)?;
                self.chunk.instructions.push(Instruction::Print);
            }
            Statement::ExprStatement(expr) => {
                self.compile_expr(expr)?;
                self.chunk.instructions.push(Instruction::Pop);
            }
            unsupported => {
                return Err(TinyLangError::runtime(format!(
                    "bytecode compiler foundation does not yet lower {unsupported:?}"
                )));
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::IntLit(value) => {
                let slot = self.push_constant(Constant::Int(*value));
                self.chunk.instructions.push(Instruction::LoadConst(slot));
            }
            Expr::StringLit(value) => {
                let slot = self.push_constant(Constant::Str(value.clone()));
                self.chunk.instructions.push(Instruction::LoadConst(slot));
            }
            Expr::BoolLit(value) => {
                let slot = self.push_constant(Constant::Bool(*value));
                self.chunk.instructions.push(Instruction::LoadConst(slot));
            }
            Expr::Ident(name) => {
                let slot = self.push_constant(Constant::Symbol(name.clone()));
                self.chunk.instructions.push(Instruction::LoadName(slot));
            }
            Expr::UnaryOp { op, operand } => {
                self.compile_expr(operand)?;
                self.chunk.instructions.push(match op {
                    UnaryOperator::Neg => Instruction::Neg,
                    UnaryOperator::Not => Instruction::Not,
                });
            }
            Expr::BinaryOp { left, op, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                self.chunk.instructions.push(match op {
                    BinaryOperator::Add => Instruction::Add,
                    BinaryOperator::Sub => Instruction::Sub,
                    BinaryOperator::Mul => Instruction::Mul,
                    BinaryOperator::Div => Instruction::Div,
                    BinaryOperator::Mod => Instruction::Mod,
                    BinaryOperator::Eq => Instruction::Equal,
                    BinaryOperator::Ne => Instruction::NotEqual,
                    BinaryOperator::Lt => Instruction::Less,
                    BinaryOperator::Gt => Instruction::Greater,
                    BinaryOperator::Le => Instruction::LessEqual,
                    BinaryOperator::Ge => Instruction::GreaterEqual,
                    BinaryOperator::And | BinaryOperator::Or => {
                        return Err(TinyLangError::runtime(
                            "bytecode compiler foundation does not yet lower short-circuit operators",
                        ));
                    }
                });
            }
            unsupported => {
                return Err(TinyLangError::runtime(format!(
                    "bytecode compiler foundation does not yet lower expression {unsupported:?}"
                )));
            }
        }
        Ok(())
    }

    fn push_constant(&mut self, constant: Constant) -> usize {
        self.chunk.constants.push(constant);
        self.chunk.constants.len() - 1
    }
}

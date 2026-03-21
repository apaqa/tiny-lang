//! Bytecode compiler。
//!
//! 這一版會把 AST 編譯成 Chunk，交給 VM 執行。
//! 編譯器採用 stack machine 模型，區域變數以 slot index 管理。

use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{BinaryOperator, Expr, MatchArm, Pattern, Program, Statement, TypeAnnotation, UnaryOperator};
use crate::environment::{CompiledFunction, StructDef, Value};
use crate::error::{Result, TinyLangError};

/// VM 指令集。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpCode {
    Constant(usize),
    Pop,
    GetLocal(usize),
    SetLocal(usize),
    GetGlobal(String),
    SetGlobal(String),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Not,
    Negate,
    Jump(usize),
    JumpIfFalse(usize),
    Loop(usize),
    Call(usize),
    Return,
    Print,
    MakeArray(usize),
    MakeMap(usize),
    Index,
    SetIndex,
    GetField(String),
    SetField(String),
    MakeStruct(String, usize),
    RuntimeError(String),
    Halt,
}

/// 編譯結果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
    pub structs: HashMap<String, StructDef>,
    pub methods: HashMap<String, HashMap<String, Rc<CompiledFunction>>>,
}

#[derive(Debug, Clone)]
struct Local {
    name: String,
    depth: usize,
}

#[derive(Debug, Clone)]
struct LoopContext {
    scope_depth: usize,
    continue_jumps: Vec<usize>,
    break_jumps: Vec<usize>,
}

/// Slot-based compiler。
#[derive(Debug, Default)]
pub struct Compiler {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: usize,
    loop_stack: Vec<LoopContext>,
    max_local_count: usize,
}

impl Compiler {
    pub fn compile_program(program: &Program) -> Result<Chunk> {
        let mut compiler = Self::default();
        for statement in program {
            compiler.compile_statement(statement)?;
        }
        compiler.emit(OpCode::Halt);
        Ok(compiler.chunk)
    }

    fn new_function(
        _function_name: Option<String>,
        params: Vec<(String, Option<TypeAnnotation>)>,
        _return_type: Option<TypeAnnotation>,
        takes_self: bool,
        inherited_structs: HashMap<String, StructDef>,
        inherited_methods: HashMap<String, HashMap<String, Rc<CompiledFunction>>>,
    ) -> Self {
        let mut compiler = Self {
            chunk: Chunk {
                code: Vec::new(),
                constants: Vec::new(),
                lines: Vec::new(),
                structs: inherited_structs,
                methods: inherited_methods,
            },
            locals: Vec::new(),
            scope_depth: 1,
            loop_stack: Vec::new(),
            max_local_count: 0,
        };

        if takes_self {
            compiler.add_local("self".into());
        }
        for (name, _) in params {
            compiler.add_local(name);
        }
        compiler
    }

    fn compile_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Import { .. } => Err(TinyLangError::runtime(
                "bytecode VM does not support import yet; use tree-walking interpreter",
            )),
            Statement::StructDecl { name, fields } => {
                self.chunk.structs.insert(
                    name.clone(),
                    StructDef {
                        name: name.clone(),
                        fields: fields.clone(),
                    },
                );
                Ok(())
            }
            Statement::MethodDecl {
                struct_name,
                method_name,
                params,
                body,
                return_type,
            } => {
                let function = self.compile_function(
                    Some(format!("{struct_name}.{method_name}")),
                    params.clone(),
                    return_type.clone(),
                    body,
                    true,
                )?;
                self.chunk
                    .methods
                    .entry(struct_name.clone())
                    .or_default()
                    .insert(method_name.clone(), function);
                Ok(())
            }
            Statement::LetDecl {
                name,
                type_annotation: _,
                value,
            } => {
                self.compile_expr(value)?;
                if self.scope_depth == 0 {
                    self.emit(OpCode::SetGlobal(name.clone()));
                } else {
                    self.add_local(name.clone());
                }
                Ok(())
            }
            Statement::Assignment { name, value } => {
                self.compile_expr(value)?;
                if let Some(slot) = self.resolve_local(name) {
                    self.emit(OpCode::SetLocal(slot));
                } else {
                    self.emit(OpCode::SetGlobal(name.clone()));
                }
                Ok(())
            }
            Statement::IndexAssignment { target, index, value } => {
                self.compile_expr(target)?;
                self.compile_expr(index)?;
                self.compile_expr(value)?;
                self.emit(OpCode::SetIndex);
                Ok(())
            }
            Statement::FieldAssignment { object, field, value } => {
                self.compile_expr(object)?;
                self.compile_expr(value)?;
                self.emit(OpCode::SetField(field.clone()));
                Ok(())
            }
            Statement::FnDecl {
                name,
                params,
                return_type,
                body,
            } => {
                let function =
                    self.compile_function(Some(name.clone()), params.clone(), return_type.clone(), body, false)?;
                let constant = self.push_constant(Value::CompiledFunction(function));
                self.emit(OpCode::Constant(constant));
                if self.scope_depth == 0 {
                    self.emit(OpCode::SetGlobal(name.clone()));
                } else {
                    self.add_local(name.clone());
                }
                Ok(())
            }
            Statement::Return(expr) => {
                self.compile_expr(expr)?;
                self.emit(OpCode::Return);
                Ok(())
            }
            Statement::IfElse {
                condition,
                then_body,
                else_body,
            } => self.compile_if_else(condition, then_body, else_body.as_deref()),
            Statement::While { condition, body } => self.compile_while(condition, body),
            Statement::ForLoop {
                variable,
                iterable,
                body,
            } => self.compile_for_loop(variable, iterable, body),
            Statement::Break => self.compile_break(),
            Statement::Continue => self.compile_continue(),
            Statement::TryCatch { .. } => Err(TinyLangError::runtime(
                "bytecode VM does not support try/catch yet; use tree-walking interpreter",
            )),
            Statement::Match { expr, arms } => self.compile_match(expr, arms),
            Statement::Print(expr) => {
                self.compile_expr(expr)?;
                self.emit(OpCode::Print);
                Ok(())
            }
            Statement::ExprStatement(expr) => {
                self.compile_expr(expr)?;
                self.emit(OpCode::Pop);
                Ok(())
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::IntLit(value) => {
                let constant = self.push_constant(Value::Int(*value));
                self.emit(OpCode::Constant(constant));
            }
            Expr::StringLit(value) => {
                let constant = self.push_constant(Value::String(value.clone()));
                self.emit(OpCode::Constant(constant));
            }
            Expr::BoolLit(value) => {
                let constant = self.push_constant(Value::Bool(*value));
                self.emit(OpCode::Constant(constant));
            }
            Expr::Ident(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    self.emit(OpCode::GetLocal(slot));
                } else {
                    self.emit(OpCode::GetGlobal(name.clone()));
                }
            }
            Expr::StructInit { name, fields } => self.compile_struct_init(name, fields)?,
            Expr::ArrayLit(items) => {
                for item in items {
                    self.compile_expr(item)?;
                }
                self.emit(OpCode::MakeArray(items.len()));
            }
            Expr::MapLit(items) => {
                for (key, value) in items {
                    self.compile_expr(key)?;
                    self.compile_expr(value)?;
                }
                self.emit(OpCode::MakeMap(items.len()));
            }
            Expr::IndexAccess { target, index } => {
                self.compile_expr(target)?;
                self.compile_expr(index)?;
                self.emit(OpCode::Index);
            }
            Expr::FieldAccess { object, field } => {
                self.compile_expr(object)?;
                self.emit(OpCode::GetField(field.clone()));
            }
            Expr::BinaryOp { left, op, right } => match op {
                BinaryOperator::And => self.compile_logical_and(left, right)?,
                BinaryOperator::Or => self.compile_logical_or(left, right)?,
                _ => {
                    self.compile_expr(left)?;
                    self.compile_expr(right)?;
                    self.emit(match op {
                        BinaryOperator::Add => OpCode::Add,
                        BinaryOperator::Sub => OpCode::Sub,
                        BinaryOperator::Mul => OpCode::Mul,
                        BinaryOperator::Div => OpCode::Div,
                        BinaryOperator::Mod => OpCode::Mod,
                        BinaryOperator::Eq => OpCode::Equal,
                        BinaryOperator::Ne => OpCode::NotEqual,
                        BinaryOperator::Lt => OpCode::Less,
                        BinaryOperator::Gt => OpCode::Greater,
                        BinaryOperator::Le => OpCode::LessEqual,
                        BinaryOperator::Ge => OpCode::GreaterEqual,
                        BinaryOperator::And | BinaryOperator::Or => unreachable!(),
                    });
                }
            },
            Expr::UnaryOp { op, operand } => {
                self.compile_expr(operand)?;
                self.emit(match op {
                    UnaryOperator::Neg => OpCode::Negate,
                    UnaryOperator::Not => OpCode::Not,
                });
            }
            Expr::FnCall { callee, args } => {
                self.compile_expr(callee)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(OpCode::Call(args.len()));
            }
            Expr::Lambda { .. } => {
                return Err(TinyLangError::runtime(
                    "bytecode VM does not support lambda/closure yet; use tree-walking interpreter",
                ));
            }
        }
        Ok(())
    }

    fn compile_if_else(
        &mut self,
        condition: &Expr,
        then_body: &[Statement],
        else_body: Option<&[Statement]>,
    ) -> Result<()> {
        self.compile_expr(condition)?;
        let else_jump = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
        self.emit(OpCode::Pop);
        self.begin_scope();
        for statement in then_body {
            self.compile_statement(statement)?;
        }
        self.end_scope();

        if let Some(else_body) = else_body {
            let end_jump = self.emit_jump(OpCode::Jump(usize::MAX));
            self.patch_jump(else_jump, self.chunk.code.len());
            self.emit(OpCode::Pop);
            self.begin_scope();
            for statement in else_body {
                self.compile_statement(statement)?;
            }
            self.end_scope();
            self.patch_jump(end_jump, self.chunk.code.len());
        } else {
            self.patch_jump(else_jump, self.chunk.code.len());
            self.emit(OpCode::Pop);
        }
        Ok(())
    }

    fn compile_while(&mut self, condition: &Expr, body: &[Statement]) -> Result<()> {
        let loop_start = self.chunk.code.len();
        self.compile_expr(condition)?;
        let exit_jump = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
        self.emit(OpCode::Pop);

        self.loop_stack.push(LoopContext {
            scope_depth: self.scope_depth,
            continue_jumps: Vec::new(),
            break_jumps: Vec::new(),
        });

        self.begin_scope();
        for statement in body {
            self.compile_statement(statement)?;
        }
        self.end_scope();

        let continue_target = self.chunk.code.len();
        self.patch_continue_jumps(continue_target);
        self.emit(OpCode::Loop(loop_start));
        self.patch_jump(exit_jump, self.chunk.code.len());
        self.emit(OpCode::Pop);
        self.patch_break_jumps(self.chunk.code.len());
        self.loop_stack.pop();
        Ok(())
    }

    fn compile_for_loop(&mut self, variable: &str, iterable: &Expr, body: &[Statement]) -> Result<()> {
        self.begin_scope();

        self.compile_expr(iterable)?;
        let iterable_slot = self.add_hidden_local("__iter");

        let zero = self.push_constant(Value::Int(0));
        self.emit(OpCode::Constant(zero));
        let index_slot = self.add_hidden_local("__index");

        let loop_start = self.chunk.code.len();

        self.emit(OpCode::GetLocal(index_slot));
        self.emit(OpCode::GetGlobal("len".into()));
        self.emit(OpCode::GetLocal(iterable_slot));
        self.emit(OpCode::Call(1));
        self.emit(OpCode::Less);

        let exit_jump = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
        self.emit(OpCode::Pop);

        self.loop_stack.push(LoopContext {
            scope_depth: self.scope_depth,
            continue_jumps: Vec::new(),
            break_jumps: Vec::new(),
        });

        self.begin_scope();
        self.emit(OpCode::GetLocal(iterable_slot));
        self.emit(OpCode::GetLocal(index_slot));
        self.emit(OpCode::Index);
        self.add_local(variable.to_string());

        for statement in body {
            self.compile_statement(statement)?;
        }
        self.end_scope();

        let continue_target = self.chunk.code.len();
        self.patch_continue_jumps(continue_target);

        self.emit(OpCode::GetLocal(index_slot));
        let one = self.push_constant(Value::Int(1));
        self.emit(OpCode::Constant(one));
        self.emit(OpCode::Add);
        self.emit(OpCode::SetLocal(index_slot));

        self.emit(OpCode::Loop(loop_start));
        self.patch_jump(exit_jump, self.chunk.code.len());
        self.emit(OpCode::Pop);
        self.patch_break_jumps(self.chunk.code.len());
        self.loop_stack.pop();
        self.end_scope();
        Ok(())
    }

    fn compile_break(&mut self) -> Result<()> {
        let Some(loop_ctx) = self.loop_stack.last().cloned() else {
            return Err(TinyLangError::runtime("break can only appear inside a loop"));
        };
        self.emit_loop_scope_cleanup(loop_ctx.scope_depth);
        let jump = self.emit_jump(OpCode::Jump(usize::MAX));
        if let Some(current) = self.loop_stack.last_mut() {
            current.break_jumps.push(jump);
        }
        Ok(())
    }

    fn compile_continue(&mut self) -> Result<()> {
        let Some(loop_ctx) = self.loop_stack.last().cloned() else {
            return Err(TinyLangError::runtime("continue can only appear inside a loop"));
        };
        self.emit_loop_scope_cleanup(loop_ctx.scope_depth);
        let jump = self.emit_jump(OpCode::Jump(usize::MAX));
        if let Some(current) = self.loop_stack.last_mut() {
            current.continue_jumps.push(jump);
        }
        Ok(())
    }

    fn compile_match(&mut self, expr: &Expr, arms: &[MatchArm]) -> Result<()> {
        self.begin_scope();
        self.compile_expr(expr)?;
        let match_slot = self.add_hidden_local("__match");
        let mut end_jumps = Vec::new();

        for arm in arms {
            match &arm.pattern {
                Pattern::IntLit(value) => {
                    self.emit(OpCode::GetLocal(match_slot));
                    let constant = self.push_constant(Value::Int(*value));
                    self.emit(OpCode::Constant(constant));
                    self.emit(OpCode::Equal);
                    let next_arm = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
                    self.emit(OpCode::Pop);
                    self.begin_scope();
                    for statement in &arm.body {
                        self.compile_statement(statement)?;
                    }
                    self.end_scope();
                    end_jumps.push(self.emit_jump(OpCode::Jump(usize::MAX)));
                    self.patch_jump(next_arm, self.chunk.code.len());
                    self.emit(OpCode::Pop);
                }
                Pattern::StringLit(value) => {
                    self.emit(OpCode::GetLocal(match_slot));
                    let constant = self.push_constant(Value::String(value.clone()));
                    self.emit(OpCode::Constant(constant));
                    self.emit(OpCode::Equal);
                    let next_arm = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
                    self.emit(OpCode::Pop);
                    self.begin_scope();
                    for statement in &arm.body {
                        self.compile_statement(statement)?;
                    }
                    self.end_scope();
                    end_jumps.push(self.emit_jump(OpCode::Jump(usize::MAX)));
                    self.patch_jump(next_arm, self.chunk.code.len());
                    self.emit(OpCode::Pop);
                }
                Pattern::BoolLit(value) => {
                    self.emit(OpCode::GetLocal(match_slot));
                    let constant = self.push_constant(Value::Bool(*value));
                    self.emit(OpCode::Constant(constant));
                    self.emit(OpCode::Equal);
                    let next_arm = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
                    self.emit(OpCode::Pop);
                    self.begin_scope();
                    for statement in &arm.body {
                        self.compile_statement(statement)?;
                    }
                    self.end_scope();
                    end_jumps.push(self.emit_jump(OpCode::Jump(usize::MAX)));
                    self.patch_jump(next_arm, self.chunk.code.len());
                    self.emit(OpCode::Pop);
                }
                Pattern::Ident(name) => {
                    self.begin_scope();
                    self.emit(OpCode::GetLocal(match_slot));
                    self.add_local(name.clone());
                    for statement in &arm.body {
                        self.compile_statement(statement)?;
                    }
                    self.end_scope();
                    end_jumps.push(self.emit_jump(OpCode::Jump(usize::MAX)));
                }
                Pattern::Wildcard => {
                    self.begin_scope();
                    for statement in &arm.body {
                        self.compile_statement(statement)?;
                    }
                    self.end_scope();
                    end_jumps.push(self.emit_jump(OpCode::Jump(usize::MAX)));
                }
            }
        }

        self.emit(OpCode::RuntimeError("match expression did not match any arm".into()));
        let end = self.chunk.code.len();
        for jump in end_jumps {
            self.patch_jump(jump, end);
        }
        self.end_scope();
        Ok(())
    }

    fn compile_logical_and(&mut self, left: &Expr, right: &Expr) -> Result<()> {
        self.compile_expr(left)?;
        let false_jump = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
        self.emit(OpCode::Pop);
        self.compile_expr(right)?;
        self.emit(OpCode::Not);
        self.emit(OpCode::Not);
        let end_jump = self.emit_jump(OpCode::Jump(usize::MAX));
        self.patch_jump(false_jump, self.chunk.code.len());
        self.emit(OpCode::Pop);
        let constant = self.push_constant(Value::Bool(false));
        self.emit(OpCode::Constant(constant));
        self.patch_jump(end_jump, self.chunk.code.len());
        Ok(())
    }

    fn compile_logical_or(&mut self, left: &Expr, right: &Expr) -> Result<()> {
        self.compile_expr(left)?;
        let else_jump = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
        self.emit(OpCode::Pop);
        let constant = self.push_constant(Value::Bool(true));
        self.emit(OpCode::Constant(constant));
        let end_jump = self.emit_jump(OpCode::Jump(usize::MAX));
        self.patch_jump(else_jump, self.chunk.code.len());
        self.emit(OpCode::Pop);
        self.compile_expr(right)?;
        self.emit(OpCode::Not);
        self.emit(OpCode::Not);
        self.patch_jump(end_jump, self.chunk.code.len());
        Ok(())
    }

    fn compile_struct_init(&mut self, name: &str, fields: &[(String, Expr)]) -> Result<()> {
        let struct_def = self
            .chunk
            .structs
            .get(name)
            .cloned()
            .ok_or_else(|| TinyLangError::runtime(format!("Struct '{name}' not defined")))?;

        for (field_name, _) in &struct_def.fields {
            let (_, expr) = fields
                .iter()
                .find(|(name, _)| name == field_name)
                .ok_or_else(|| TinyLangError::runtime(format!("Struct '{name}' initialization missing field '{field_name}'")))?;
            self.compile_expr(expr)?;
        }

        for (field_name, _) in fields {
            if !struct_def.fields.iter().any(|(declared, _)| declared == field_name) {
                return Err(TinyLangError::runtime(format!(
                    "Struct '{}' has no field '{}'",
                    name, field_name
                )));
            }
        }

        self.emit(OpCode::MakeStruct(name.to_string(), struct_def.fields.len()));
        Ok(())
    }

    fn compile_function(
        &self,
        name: Option<String>,
        params: Vec<(String, Option<TypeAnnotation>)>,
        return_type: Option<TypeAnnotation>,
        body: &[Statement],
        takes_self: bool,
    ) -> Result<Rc<CompiledFunction>> {
        let mut compiler = Compiler::new_function(
            name.clone(),
            params.clone(),
            return_type.clone(),
            takes_self,
            self.chunk.structs.clone(),
            self.chunk.methods.clone(),
        );

        for statement in body {
            compiler.compile_statement(statement)?;
        }

        let null_constant = compiler.push_constant(Value::Null);
        compiler.emit(OpCode::Constant(null_constant));
        compiler.emit(OpCode::Return);

        Ok(Rc::new(CompiledFunction {
            name,
            params,
            return_type,
            chunk: Rc::new(compiler.chunk),
            local_count: compiler.max_local_count,
            takes_self,
        }))
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        while matches!(self.locals.last(), Some(local) if local.depth == self.scope_depth) {
            self.emit(OpCode::Pop);
            self.locals.pop();
        }
        self.scope_depth = self.scope_depth.saturating_sub(1);
    }

    fn add_local(&mut self, name: String) -> usize {
        let slot = self.locals.len();
        self.locals.push(Local {
            name,
            depth: self.scope_depth,
        });
        self.max_local_count = self.max_local_count.max(self.locals.len());
        slot
    }

    fn add_hidden_local(&mut self, prefix: &str) -> usize {
        self.add_local(format!("{prefix}@{}", self.locals.len()))
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.locals.iter().rposition(|local| local.name == name)
    }

    fn emit(&mut self, opcode: OpCode) {
        self.chunk.code.push(opcode);
        self.chunk.lines.push(0);
    }

    fn emit_jump(&mut self, opcode: OpCode) -> usize {
        let index = self.chunk.code.len();
        self.emit(opcode);
        index
    }

    fn patch_jump(&mut self, instruction_index: usize, target: usize) {
        match &mut self.chunk.code[instruction_index] {
            OpCode::Jump(destination) | OpCode::JumpIfFalse(destination) => *destination = target,
            _ => panic!("attempted to patch non-jump opcode"),
        }
    }

    fn patch_continue_jumps(&mut self, target: usize) {
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            let jumps = std::mem::take(&mut loop_ctx.continue_jumps);
            for jump in jumps {
                self.patch_jump(jump, target);
            }
        }
    }

    fn patch_break_jumps(&mut self, target: usize) {
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            let jumps = std::mem::take(&mut loop_ctx.break_jumps);
            for jump in jumps {
                self.patch_jump(jump, target);
            }
        }
    }

    fn emit_loop_scope_cleanup(&mut self, loop_scope_depth: usize) {
        let extra_locals = self
            .locals
            .iter()
            .filter(|local| local.depth > loop_scope_depth)
            .count();
        for _ in 0..extra_locals {
            self.emit(OpCode::Pop);
        }
    }

    fn push_constant(&mut self, value: Value) -> usize {
        self.chunk.constants.push(value);
        self.chunk.constants.len() - 1
    }
}

/// 把 chunk 反組譯成人類可讀的文字格式。
pub fn disassemble(chunk: &Chunk) -> String {
    let mut lines = Vec::new();
    for (index, opcode) in chunk.code.iter().enumerate() {
        let source_line = chunk.lines.get(index).copied().unwrap_or(0);
        lines.push(format!("{index:04} [line {source_line:>3}] {opcode:?}"));
    }
    lines.join("\n")
}

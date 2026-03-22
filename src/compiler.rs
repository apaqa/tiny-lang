//! Bytecode compiler。
//!
//! 這一版會把 AST 編譯成 Chunk，交給 VM 執行。
//! 編譯器採用 stack machine 模型，區域變數以 slot index 管理。
//! Phase 7 新增：閉包 upvalue 分析與編譯、enum 支援。

use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::ast::{
    BinaryOperator, Expr, MatchArm, Pattern, Program, Statement, TypeAnnotation, UnaryOperator,
};
use crate::environment::{CaptureSource, CompiledFunction, StructDef, Value};
use crate::error::{Result, TinyLangError};
use crate::gc::GcHeap;

/// VM 指令集。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpCode {
    Constant(usize),
    Pop,
    GetLocal(usize),
    SetLocal(usize),
    GetGlobal(String),
    SetGlobal(String),
    /// 讀取當前 frame 的第 i 個 upvalue（閉包捕獲的外部變數）
    GetUpvalue(usize),
    /// 寫入當前 frame 的第 i 個 upvalue
    SetUpvalue(usize),
    /// 關閉 upvalue（被捕獲的 local 離開作用域時發出）
    CloseUpvalue,
    /// 從常數池建立閉包，並從 stack 上取得 capture_names.len() 個捕獲值
    MakeClosure(usize),
    /// 中文註解：進入 try 區塊時記住 catch 的跳轉位置。
    TryBegin(usize),
    /// 中文註解：正常離開 try 區塊時移除對應的 try frame。
    TryEnd,
    /// 中文註解：主動拋出目前 stack 頂端的錯誤字串。
    Throw,
    /// 中文註解：在 VM 內載入並執行另一個 tiny 檔案。
    Import(String),
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
    /// 建立 enum variant 值，field_names 依序對應 stack 上的值
    MakeEnumVariant(String, String, Vec<String>),
    /// 彈出頂部值，檢查是否為指定 enum variant，推入 bool
    CheckEnumVariant(String, String),
    /// 彈出 enum variant，推入指定欄位的值
    GetEnumField(String),
    RuntimeError(String),
    Halt,
}

/// 編譯結果。
#[derive(Debug)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
    pub structs: HashMap<String, StructDef>,
    pub methods: HashMap<String, HashMap<String, Rc<CompiledFunction>>>,
    pub heap: GcHeap,
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
            structs: HashMap::new(),
            methods: HashMap::new(),
            heap: GcHeap::new(),
        }
    }
}

impl PartialEq for Chunk {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.constants == other.constants
            && self.lines == other.lines
            && self.structs == other.structs
    }
}

impl Eq for Chunk {}

/// 區域變數資訊。
#[derive(Debug, Clone)]
struct Local {
    name: String,
    depth: usize,
    /// 是否被內部閉包捕獲
    captured: bool,
}

#[derive(Debug, Clone)]
struct LoopContext {
    scope_depth: usize,
    continue_jumps: Vec<usize>,
    break_jumps: Vec<usize>,
}

/// Slot-based compiler。
#[derive(Debug)]
pub struct Compiler {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: usize,
    loop_stack: Vec<LoopContext>,
    max_local_count: usize,
    /// 當前函式所捕獲的 upvalue 名稱列表（只在閉包 compiler 中有值）
    upvalue_names: Vec<String>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self {
            chunk: Chunk::default(),
            locals: Vec::new(),
            scope_depth: 0,
            loop_stack: Vec::new(),
            max_local_count: 0,
            upvalue_names: Vec::new(),
        }
    }
}

impl Compiler {
    pub fn compile_program(program: &Program) -> Result<Chunk> {
        Self::compile_program_with_heap(program, GcHeap::new())
    }

    pub fn compile_program_with_heap(program: &Program, heap: GcHeap) -> Result<Chunk> {
        let mut compiler = Self::default();
        // 中文註解：讓編譯出來的常數直接落在既有 heap，import 時就能和目前 VM 共用物件。
        compiler.chunk.heap = heap;
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
                heap: GcHeap::new(),
            },
            locals: Vec::new(),
            scope_depth: 1,
            loop_stack: Vec::new(),
            max_local_count: 0,
            upvalue_names: Vec::new(),
        };

        if takes_self {
            compiler.add_local("self".into());
        }
        for (name, _) in params {
            compiler.add_local(name);
        }
        compiler
    }

    /// 建立一個帶有 upvalue 的閉包 compiler（用於 lambda 編譯）。
    fn new_closure(
        params: Vec<(String, Option<TypeAnnotation>)>,
        takes_self: bool,
        inherited_structs: HashMap<String, StructDef>,
        inherited_methods: HashMap<String, HashMap<String, Rc<CompiledFunction>>>,
        capture_names: Vec<String>,
    ) -> Self {
        let mut compiler = Self {
            chunk: Chunk {
                code: Vec::new(),
                constants: Vec::new(),
                lines: Vec::new(),
                structs: inherited_structs,
                methods: inherited_methods,
                heap: GcHeap::new(),
            },
            locals: Vec::new(),
            scope_depth: 1,
            loop_stack: Vec::new(),
            max_local_count: 0,
            // 設定 upvalue 名稱，讓 Ident 解析能識別捕獲變數
            upvalue_names: capture_names,
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
            Statement::Import { path } => {
                self.emit(OpCode::Import(path.clone()));
                Ok(())
            }
            Statement::InterfaceDecl { .. } => Ok(()),
            Statement::ImplInterface {
                struct_name, methods, ..
            } => {
                // VM 目前不追蹤 interface metadata，但仍需要把 impl 內的方法編進 method table。
                for method in methods {
                    let Statement::FnDecl {
                        name,
                        type_params: _,
                        params,
                        return_type,
                        body,
                    } = method
                    else {
                        continue;
                    };
                    let method_params = params.iter().skip(1).cloned().collect();
                    let function = self.compile_function(
                        Some(format!("{struct_name}.{name}")),
                        method_params,
                        return_type.clone(),
                        body,
                        true,
                    )?;
                    self.chunk
                        .methods
                        .entry(struct_name.clone())
                        .or_default()
                        .insert(name.clone(), function);
                }
                Ok(())
            }
            Statement::EnumDecl { .. } => {
                // VM 模式下 enum 定義不需要生成 bytecode
                // enum variant 的建立由 MakeEnumVariant 指令處理
                Ok(())
            }
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
                let method_params = strip_self_param(params);
                let function = self.compile_function(
                    Some(format!("{struct_name}.{method_name}")),
                    method_params,
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
                // 先檢查是否是 local 變數
                if let Some(slot) = self.resolve_local(name) {
                    self.emit(OpCode::SetLocal(slot));
                } else if let Some(idx) = self.resolve_upvalue(name) {
                    // 再檢查是否是捕獲的 upvalue（閉包內修改外部變數）
                    self.emit(OpCode::SetUpvalue(idx));
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
                type_params: _,
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
            Statement::AsyncFnDecl {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                // 中文註解：VM 模式下將 async fn 編譯為普通函式（await 在 VM 中直接求值）。
                let function = self.compile_function(Some(name.clone()), params.clone(), return_type.clone(), body, false)?;
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
            Statement::TryCatch {
                try_body,
                catch_var,
                catch_body,
            } => self.compile_try_catch(try_body, catch_var, catch_body),
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
                let str_ref = self.chunk.heap.alloc_string(value.clone());
                let constant = self.push_constant(Value::String(str_ref));
                self.emit(OpCode::Constant(constant));
            }
            Expr::BoolLit(value) => {
                let constant = self.push_constant(Value::Bool(*value));
                self.emit(OpCode::Constant(constant));
            }
            Expr::NullLit => {
                let constant = self.push_constant(Value::Null);
                self.emit(OpCode::Constant(constant));
            }
            Expr::Ident(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    // 先查 local 變數
                    self.emit(OpCode::GetLocal(slot));
                } else if let Some(idx) = self.resolve_upvalue(name) {
                    // 再查 upvalue（閉包捕獲的外部變數）
                    self.emit(OpCode::GetUpvalue(idx));
                } else {
                    // 最後查 global
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
            Expr::Lambda { params, body } => {
                // 編譯 lambda：分析捕獲的外部變數，生成 MakeClosure 指令
                self.compile_lambda(params, body)?;
            }
            Expr::Await { expr } => {
                // 中文註解：VM 模式下 await 直接編譯內部表達式（不產生 Future 包裝）。
                self.compile_expr(expr)?;
            }
            Expr::EnumVariant { enum_name, variant, fields } => {
                // 編譯 enum variant 建立：推送所有欄位值，發出 MakeEnumVariant
                let mut field_names = Vec::new();
                if let Some(fields) = fields {
                    for (field_name, expr) in fields {
                        self.compile_expr(expr)?;
                        field_names.push(field_name.clone());
                    }
                }
                self.emit(OpCode::MakeEnumVariant(enum_name.clone(), variant.clone(), field_names));
            }
        }
        Ok(())
    }

    /// 編譯 lambda 表達式，分析捕獲的外部 local 變數，生成 upvalue。
    fn compile_lambda(&mut self, params: &[String], body: &[Statement]) -> Result<()> {
        // 收集 lambda 體中所有引用的識別符
        let all_refs = collect_body_idents(body);
        let param_set: HashSet<&str> = params.iter().map(|p| p.as_str()).collect();

        // 找出在外部作用域 locals 中定義的識別符 → 這些是需要捕獲的 upvalue
        let mut captures: Vec<(String, CaptureSource)> = all_refs
            .iter()
            .filter(|name| !param_set.contains(name.as_str()))
            .filter_map(|name| {
                if let Some(slot) = self.resolve_local(name) {
                    Some((name.clone(), CaptureSource::Local(slot)))
                } else {
                    self.resolve_upvalue(name)
                        .map(|index| (name.clone(), CaptureSource::Upvalue(index)))
                }
            })
            .collect();

        // 按 slot 排序確保一致的順序，避免測試因集合順序飄動
        captures.sort_by(|left, right| match (&left.1, &right.1) {
            (CaptureSource::Local(a), CaptureSource::Local(b)) => a.cmp(b).then(left.0.cmp(&right.0)),
            (CaptureSource::Upvalue(a), CaptureSource::Upvalue(b)) => {
                a.cmp(b).then(left.0.cmp(&right.0))
            }
            (CaptureSource::Local(_), CaptureSource::Upvalue(_)) => std::cmp::Ordering::Less,
            (CaptureSource::Upvalue(_), CaptureSource::Local(_)) => std::cmp::Ordering::Greater,
        });
        captures.dedup_by(|left, right| left.1 == right.1);

        let capture_names: Vec<String> = captures.iter().map(|(name, _)| name.clone()).collect();
        let capture_sources: Vec<CaptureSource> =
            captures.iter().map(|(_, source)| source.clone()).collect();

        // 標記被捕獲的 locals，讓 end_scope 生成 CloseUpvalue 而非 Pop
        for (_, source) in &captures {
            if let CaptureSource::Local(slot) = source {
                if let Some(local) = self.locals.get_mut(*slot) {
                    local.captured = true;
                }
            }
        }

        // 建立 lambda 的參數列表（lambda 沒有型別註記）
        let param_pairs: Vec<(String, Option<TypeAnnotation>)> =
            params.iter().map(|p| (p.clone(), None)).collect();

        // 編譯閉包函式（使用帶 upvalue_names 的內部 compiler）
        let function = self.compile_closure_function(
            None,
            param_pairs,
            None,
            body,
            false,
            capture_names.clone(),
            capture_sources,
        )?;

        let const_idx = self.push_constant(Value::CompiledFunction(function.clone()));

        if function.capture_sources.is_empty() {
            // 無捕獲變數：直接作為普通函式常數使用（不需要 Closure 包裝）
            self.emit(OpCode::Constant(const_idx));
        } else {
            // 有捕獲變數：發出 MakeClosure，VM 端依照 capture_sources 建立 Closure 物件
            self.emit(OpCode::MakeClosure(const_idx));
        }

        Ok(())
    }

    /// 編譯帶有 upvalue 支援的閉包函式。
    fn compile_closure_function(
        &mut self,
        name: Option<String>,
        params: Vec<(String, Option<TypeAnnotation>)>,
        return_type: Option<TypeAnnotation>,
        body: &[Statement],
        takes_self: bool,
        capture_names: Vec<String>,
        capture_sources: Vec<CaptureSource>,
    ) -> Result<Rc<CompiledFunction>> {
        let mut compiler = Compiler::new_closure(
            params.clone(),
            takes_self,
            self.chunk.structs.clone(),
            self.chunk.methods.clone(),
            capture_names.clone(),
        );

        // 與父 compiler 共享 heap，讓字串常數存在同一個 GcHeap
        std::mem::swap(&mut self.chunk.heap, &mut compiler.chunk.heap);

        for statement in body {
            compiler.compile_statement(statement)?;
        }

        // 確保函式有 return（不管有沒有 return 語句）
        let null_constant = compiler.push_constant(Value::Null);
        compiler.emit(OpCode::Constant(null_constant));
        compiler.emit(OpCode::Return);

        // 歸還 heap 給父 compiler
        std::mem::swap(&mut self.chunk.heap, &mut compiler.chunk.heap);

        Ok(Rc::new(CompiledFunction {
            name,
            params,
            return_type,
            chunk: Rc::new(compiler.chunk),
            local_count: compiler.max_local_count,
            takes_self,
            capture_names,
            capture_sources,
        }))
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

    fn compile_try_catch(
        &mut self,
        try_body: &[Statement],
        catch_var: &str,
        catch_body: &[Statement],
    ) -> Result<()> {
        let try_begin = self.emit_jump(OpCode::TryBegin(usize::MAX));

        // 中文註解：try 區塊使用獨立 scope，和 interpreter 的 block 語意保持一致。
        self.begin_scope();
        for statement in try_body {
            self.compile_statement(statement)?;
        }
        self.end_scope();

        self.emit(OpCode::TryEnd);
        let end_jump = self.emit_jump(OpCode::Jump(usize::MAX));

        let catch_ip = self.chunk.code.len();
        self.patch_jump(try_begin, catch_ip);

        // 中文註解：VM 進入 catch 前會把錯誤字串推到 stack，這裡直接把它綁到 local。
        self.begin_scope();
        self.add_local(catch_var.to_string());
        for statement in catch_body {
            self.compile_statement(statement)?;
        }
        self.end_scope();

        self.patch_jump(end_jump, self.chunk.code.len());
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
                    let str_ref = self.chunk.heap.alloc_string(value.clone());
                    let constant = self.push_constant(Value::String(str_ref));
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
                Pattern::EnumVariant { enum_name, variant, bindings } => {
                    // 推入 match 值的副本，用於 variant 名稱比對
                    self.emit(OpCode::GetLocal(match_slot));
                    // CheckEnumVariant 彈出值，推入 bool
                    self.emit(OpCode::CheckEnumVariant(enum_name.clone(), variant.clone()));
                    let next_arm = self.emit_jump(OpCode::JumpIfFalse(usize::MAX));
                    self.emit(OpCode::Pop); // 彈出 true bool
                    self.begin_scope();
                    // 如果有綁定名稱，把整個 variant 值綁定到第一個名稱
                    if let Some(binding_names) = bindings {
                        if !binding_names.is_empty() {
                            self.emit(OpCode::GetLocal(match_slot));
                            self.add_local(binding_names[0].clone());
                        }
                    }
                    for statement in &arm.body {
                        self.compile_statement(statement)?;
                    }
                    self.end_scope();
                    end_jumps.push(self.emit_jump(OpCode::Jump(usize::MAX)));
                    self.patch_jump(next_arm, self.chunk.code.len());
                    self.emit(OpCode::Pop); // 彈出 false bool
                }
            }
        }

        // 中文註解：用 Throw 走統一的錯誤攔截流程，讓 try/catch 可以接住 match 失敗。
        let error_text = self.chunk.heap.alloc_string("Runtime error: match expression did not match any arm".into());
        let constant = self.push_constant(Value::String(error_text));
        self.emit(OpCode::Constant(constant));
        self.emit(OpCode::Throw);
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
        &mut self,
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

        // Share the parent heap with the sub-compiler so all string
        // constants end up in the same GcHeap that the VM will adopt.
        std::mem::swap(&mut self.chunk.heap, &mut compiler.chunk.heap);

        for statement in body {
            compiler.compile_statement(statement)?;
        }

        let null_constant = compiler.push_constant(Value::Null);
        compiler.emit(OpCode::Constant(null_constant));
        compiler.emit(OpCode::Return);

        // Give the (possibly grown) heap back to the parent compiler.
        std::mem::swap(&mut self.chunk.heap, &mut compiler.chunk.heap);

        Ok(Rc::new(CompiledFunction {
            name,
            params,
            return_type,
            chunk: Rc::new(compiler.chunk),
            local_count: compiler.max_local_count,
            takes_self,
            capture_names: Vec::new(),
            capture_sources: Vec::new(),
        }))
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        // 彈出當前作用域的所有 local 變數
        // 被捕獲的變數發出 CloseUpvalue，其他發出 Pop
        while matches!(self.locals.last(), Some(local) if local.depth == self.scope_depth) {
            let local = self.locals.pop().unwrap();
            if local.captured {
                // 被閉包捕獲的變數，發出 CloseUpvalue 指令
                self.emit(OpCode::CloseUpvalue);
            } else {
                self.emit(OpCode::Pop);
            }
        }
        self.scope_depth = self.scope_depth.saturating_sub(1);
    }

    fn add_local(&mut self, name: String) -> usize {
        let slot = self.locals.len();
        self.locals.push(Local {
            name,
            depth: self.scope_depth,
            captured: false,
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

    /// 解析 upvalue 索引（只在閉包 compiler 中有效）。
    fn resolve_upvalue(&self, name: &str) -> Option<usize> {
        self.upvalue_names.iter().position(|n| n == name)
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
            OpCode::Jump(destination)
            | OpCode::JumpIfFalse(destination)
            | OpCode::TryBegin(destination) => *destination = target,
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

/// 收集語句列表中所有引用的識別符名稱（用於 upvalue 分析）。
fn collect_body_idents(stmts: &[Statement]) -> BTreeSet<String> {
    let mut set = HashSet::new();
    for stmt in stmts {
        collect_stmt_idents(stmt, &mut set);
    }
    set.into_iter().collect()
}

fn collect_stmt_idents(stmt: &Statement, set: &mut HashSet<String>) {
    match stmt {
        Statement::LetDecl { value, .. } => collect_expr_idents(value, set),
        Statement::Assignment { name, value } => {
            // 指定的目標變數名也需要記錄（可能是需要透過 SetUpvalue 修改的捕獲變數）
            set.insert(name.clone());
            collect_expr_idents(value, set);
        }
        Statement::IndexAssignment { target, index, value } => {
            collect_expr_idents(target, set);
            collect_expr_idents(index, set);
            collect_expr_idents(value, set);
        }
        Statement::FieldAssignment { object, value, .. } => {
            collect_expr_idents(object, set);
            collect_expr_idents(value, set);
        }
        Statement::Return(expr) | Statement::Print(expr) | Statement::ExprStatement(expr) => {
            collect_expr_idents(expr, set);
        }
        Statement::IfElse { condition, then_body, else_body } => {
            collect_expr_idents(condition, set);
            for s in then_body {
                collect_stmt_idents(s, set);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    collect_stmt_idents(s, set);
                }
            }
        }
        Statement::While { condition, body } => {
            collect_expr_idents(condition, set);
            for s in body {
                collect_stmt_idents(s, set);
            }
        }
        Statement::ForLoop { iterable, body, .. } => {
            collect_expr_idents(iterable, set);
            for s in body {
                collect_stmt_idents(s, set);
            }
        }
        Statement::Match { expr, arms } => {
            collect_expr_idents(expr, set);
            for arm in arms {
                for s in &arm.body {
                    collect_stmt_idents(s, set);
                }
            }
        }
        Statement::FnDecl { body, .. } | Statement::AsyncFnDecl { body, .. } => {
            for s in body {
                collect_stmt_idents(s, set);
            }
        }
        Statement::ImplInterface { methods, .. } => {
            for method in methods {
                if let Statement::FnDecl { body, .. } = method {
                    for s in body {
                        collect_stmt_idents(s, set);
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_expr_idents(expr: &Expr, set: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name) => {
            set.insert(name.clone());
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_expr_idents(left, set);
            collect_expr_idents(right, set);
        }
        Expr::UnaryOp { operand, .. } => collect_expr_idents(operand, set),
        Expr::FnCall { callee, args } => {
            collect_expr_idents(callee, set);
            for a in args {
                collect_expr_idents(a, set);
            }
        }
        Expr::IndexAccess { target, index } => {
            collect_expr_idents(target, set);
            collect_expr_idents(index, set);
        }
        Expr::FieldAccess { object, .. } => collect_expr_idents(object, set),
        Expr::ArrayLit(items) => {
            for i in items {
                collect_expr_idents(i, set);
            }
        }
        Expr::MapLit(items) => {
            for (k, v) in items {
                collect_expr_idents(k, set);
                collect_expr_idents(v, set);
            }
        }
        Expr::StructInit { fields, .. } => {
            for (_, e) in fields {
                collect_expr_idents(e, set);
            }
        }
        Expr::EnumVariant { fields, .. } => {
            if let Some(fields) = fields {
                for (_, e) in fields {
                    collect_expr_idents(e, set);
                }
            }
        }
        Expr::Lambda { body, .. } => {
            // 巢狀 lambda 中的引用也算外層的自由變數候選
            for s in body {
                collect_stmt_idents(s, set);
            }
        }
        Expr::Await { expr } => collect_expr_idents(expr, set),
        _ => {}
    }
}

/// 把 chunk 反組譯成人類可讀的文字格式。
fn strip_self_param(params: &[(String, Option<TypeAnnotation>)]) -> Vec<(String, Option<TypeAnnotation>)> {
    if let Some((first, rest)) = params.split_first() {
        if first.0 == "self" {
            return rest.to_vec();
        }
    }
    params.to_vec()
}

pub fn disassemble(chunk: &Chunk) -> String {
    let mut lines = Vec::new();
    for (index, opcode) in chunk.code.iter().enumerate() {
        let source_line = chunk.lines.get(index).copied().unwrap_or(0);
        lines.push(format!("{index:04} [line {source_line:>3}] {opcode:?}"));
    }
    lines.join("\n")
}

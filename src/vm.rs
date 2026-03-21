//! Bytecode VM。
//!
//! VM 會執行 compiler 產生的 Chunk，並維護 stack 與 call frame。
//! Phase 7 新增：閉包 upvalue 支援、enum 指令、自動 GC 觸發。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{Stdout, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ast::TypeAnnotation;
use crate::compiler::{Chunk, OpCode};
use crate::environment::{
    array_value, closure_value, enum_variant_value, map_value, render_value, string_value,
    struct_instance_value, CaptureSource, CompiledFunction, NativeFunction, StructDef, Value,
    VmBoundMethodValue,
};
use crate::error::{Result, TinyLangError};
use crate::gc::GcHeap;

/// 呼叫棧上的 frame。
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function: Rc<CompiledFunction>,
    pub ip: usize,
    pub slot_offset: usize,
    /// 閉包的 upvalue 列表（普通函式為空）
    pub upvalues: Vec<Rc<RefCell<Value>>>,
}

#[derive(Debug, Clone)]
struct TryFrame {
    /// 中文註解：catch block 的指令位置。
    catch_ip: usize,
    /// 中文註解：發生錯誤時要把 value stack 回滾到 try 開始前的深度。
    stack_depth: usize,
    /// 中文註解：發生錯誤時保留到哪一層 call frame。
    frame_depth: usize,
}

/// Bytecode 虛擬機。
pub struct VM<W: Write> {
    pub stack: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, Value>,
    structs: HashMap<String, StructDef>,
    methods: HashMap<String, HashMap<String, Rc<CompiledFunction>>>,
    try_stack: Vec<TryFrame>,
    open_upvalues: HashMap<usize, Rc<RefCell<Value>>>,
    frame_restore_dirs: Vec<Option<PathBuf>>,
    current_dir: PathBuf,
    imported_files: HashSet<PathBuf>,
    output: W,
    heap: GcHeap,
}

impl VM<Stdout> {
    pub fn new() -> Self {
        Self::with_output(std::io::stdout())
    }
}

impl<W: Write> VM<W> {
    pub fn with_output(output: W) -> Self {
        let mut vm = Self {
            stack: Vec::new(),
            frames: Vec::new(),
            globals: HashMap::new(),
            structs: HashMap::new(),
            methods: HashMap::new(),
            try_stack: Vec::new(),
            open_upvalues: HashMap::new(),
            frame_restore_dirs: Vec::new(),
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            imported_files: HashSet::new(),
            output,
            heap: GcHeap::new(),
        };
        vm.install_natives();
        vm
    }

    /// 執行整個 chunk。
    pub fn set_current_dir(&mut self, path: impl AsRef<Path>) {
        self.current_dir = path.as_ref().to_path_buf();
    }

    pub fn run(&mut self, chunk: Chunk) -> Result<Value> {
        // 中文註解：統一委派給 run_internal，後者處理 try/catch、import 與 upvalue。
        self.run_internal(chunk)
    }

    fn run_internal(&mut self, chunk: Chunk) -> Result<Value> {
        self.stack.clear();
        self.frames.clear();
        self.try_stack.clear();
        self.open_upvalues.clear();
        self.frame_restore_dirs.clear();
        self.structs = chunk.structs.clone();
        self.methods = chunk.methods.clone();
        self.heap = chunk.heap.clone();

        self.push_script_frame(chunk, "<script>", None);

        loop {
            let instruction = match self.next_instruction() {
                Some(instruction) => instruction,
                None => return Ok(Value::Null),
            };

            match self.execute_instruction(instruction) {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => {}
                Err(err) => {
                    if !self.handle_runtime_error(err.clone())? {
                        return Err(err);
                    }
                }
            }
        }
    }

    fn next_instruction(&mut self) -> Option<OpCode> {
        let frame = self.frames.last_mut()?;
        if frame.ip >= frame.function.chunk.code.len() {
            return Some(OpCode::Halt);
        }
        let opcode = frame.function.chunk.code[frame.ip].clone();
        frame.ip += 1;
        Some(opcode)
    }

    fn push_script_frame(&mut self, chunk: Chunk, name: &str, restore_dir: Option<PathBuf>) {
        let script = Rc::new(CompiledFunction {
            name: Some(name.into()),
            params: Vec::new(),
            return_type: None,
            chunk: Rc::new(chunk),
            local_count: 0,
            takes_self: false,
            capture_names: Vec::new(),
            capture_sources: Vec::new(),
        });

        self.frames.push(CallFrame {
            function: script,
            ip: 0,
            slot_offset: self.stack.len(),
            upvalues: Vec::new(),
        });
        self.frame_restore_dirs.push(restore_dir);
    }

    fn execute_instruction(&mut self, instruction: OpCode) -> Result<Option<Value>> {
        match instruction {
            OpCode::Constant(index) => {
                let value = self.current_chunk().constants[index].clone();
                self.stack.push(value);
            }
            OpCode::Pop => {
                self.pop_value()?;
            }
            OpCode::GetLocal(slot) => {
                let value = self.read_local(slot)?;
                self.stack.push(value);
            }
            OpCode::SetLocal(slot) => {
                let value = self.pop_value()?;
                self.write_local(slot, value)?;
            }
            OpCode::GetGlobal(name) => {
                let value = self
                    .globals
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| TinyLangError::runtime(format!("Variable '{name}' not defined")))?;
                self.stack.push(value);
            }
            OpCode::SetGlobal(name) => {
                let value = self.pop_value()?;
                self.globals.insert(name, value);
            }
            OpCode::GetUpvalue(index) => {
                let value = self
                    .current_frame()
                    .upvalues
                    .get(index)
                    .ok_or_else(|| TinyLangError::runtime(format!("Upvalue index {index} out of bounds")))?
                    .borrow()
                    .clone();
                self.stack.push(value);
            }
            OpCode::SetUpvalue(index) => {
                let value = self.pop_value()?;
                let cell = self
                    .current_frame()
                    .upvalues
                    .get(index)
                    .ok_or_else(|| TinyLangError::runtime(format!("Upvalue index {index} out of bounds")))?
                    .clone();
                *cell.borrow_mut() = value;
            }
            OpCode::CloseUpvalue => {
                self.close_top_upvalue()?;
            }
            OpCode::MakeClosure(const_idx) => {
                let function = match &self.current_chunk().constants[const_idx] {
                    Value::CompiledFunction(f) => f.clone(),
                    _ => return Err(TinyLangError::runtime("MakeClosure: expected CompiledFunction constant")),
                };
                let captured = self.capture_cells(&function)?;
                let closure = closure_value(&mut self.heap, function, captured);
                self.stack.push(closure);
                self.try_collect_garbage();
            }
            OpCode::TryBegin(catch_ip) => {
                self.try_stack.push(TryFrame {
                    catch_ip,
                    stack_depth: self.stack.len(),
                    frame_depth: self.frames.len(),
                });
            }
            OpCode::TryEnd => {
                self.try_stack.pop();
            }
            OpCode::Throw => {
                let value = self.pop_value()?;
                let message = render_value(&self.heap, &value);
                return Err(TinyLangError::runtime(message));
            }
            OpCode::Import(path) => {
                self.execute_import(&path)?;
            }
            OpCode::Add => self.execute_add()?,
            OpCode::Sub => self.execute_int_binary("subtract", |a, b| a - b)?,
            OpCode::Mul => self.execute_int_binary("multiply", |a, b| a * b)?,
            OpCode::Div => {
                let (a, b) = self.pop_int_pair("divide")?;
                if b == 0 {
                    return Err(TinyLangError::runtime("Cannot divide by zero"));
                }
                self.stack.push(Value::Int(a / b));
            }
            OpCode::Mod => {
                let (a, b) = self.pop_int_pair("modulo")?;
                if b == 0 {
                    return Err(TinyLangError::runtime("Cannot modulo by zero"));
                }
                self.stack.push(Value::Int(a % b));
            }
            OpCode::Equal => {
                let right = self.pop_value()?;
                let left = self.pop_value()?;
                self.stack.push(Value::Bool(self.values_equal(&left, &right)));
            }
            OpCode::NotEqual => {
                let right = self.pop_value()?;
                let left = self.pop_value()?;
                self.stack.push(Value::Bool(!self.values_equal(&left, &right)));
            }
            OpCode::Less => self.execute_int_compare("<", |a, b| a < b)?,
            OpCode::Greater => self.execute_int_compare(">", |a, b| a > b)?,
            OpCode::LessEqual => self.execute_int_compare("<=", |a, b| a <= b)?,
            OpCode::GreaterEqual => self.execute_int_compare(">=", |a, b| a >= b)?,
            OpCode::Not => {
                let value = self.pop_value()?;
                self.stack.push(Value::Bool(!value.is_truthy()));
            }
            OpCode::Negate => {
                let value = self.pop_value()?;
                match value {
                    Value::Int(number) => self.stack.push(Value::Int(-number)),
                    other => {
                        return Err(TinyLangError::runtime(format!(
                            "Cannot negate {}",
                            other.type_name_for_error()
                        )))
                    }
                }
            }
            OpCode::Jump(target) | OpCode::Loop(target) => {
                self.current_frame_mut().ip = target;
            }
            OpCode::JumpIfFalse(target) => {
                if !self.peek_value()?.is_truthy() {
                    self.current_frame_mut().ip = target;
                }
            }
            OpCode::Call(arg_count) => self.execute_call(arg_count)?,
            OpCode::Return => {
                let value = self.handle_return()?;
                // handle_return 已將回傳值推回 stack（巢狀呼叫）或清空 stack（頂層）。
                // 只有 frames 已空（從頂層 script 回傳）時，才結束整個 VM 執行迴圈。
                if self.frames.is_empty() {
                    return Ok(Some(value));
                }
            }
            OpCode::Print => {
                let value = self.pop_value()?;
                let rendered = render_value(&self.heap, &value);
                writeln!(self.output, "{rendered}").map_err(|err| TinyLangError::io(err.to_string()))?;
            }
            OpCode::MakeArray(count) => {
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.pop_value()?);
                }
                items.reverse();
                self.stack.push(array_value(&mut self.heap, items));
                self.try_collect_garbage();
            }
            OpCode::MakeMap(count) => {
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    let value = self.pop_value()?;
                    let key = self.pop_value()?;
                    entries.push((self.expect_map_key(key)?, value));
                }
                entries.reverse();
                let mut map = HashMap::new();
                for (key, value) in entries {
                    map.insert(key, value);
                }
                self.stack.push(map_value(&mut self.heap, map));
                self.try_collect_garbage();
            }
            OpCode::Index => {
                let index = self.pop_value()?;
                let target = self.pop_value()?;
                let val = self.read_index(target, index)?;
                self.stack.push(val);
            }
            OpCode::SetIndex => {
                let value = self.pop_value()?;
                let index = self.pop_value()?;
                let target = self.pop_value()?;
                self.assign_index(target, index, value)?;
            }
            OpCode::GetField(field) => {
                let object = self.pop_value()?;
                self.stack.push(self.read_field_or_method(object, &field)?);
            }
            OpCode::SetField(field) => {
                let value = self.pop_value()?;
                let object = self.pop_value()?;
                self.assign_field(object, &field, value)?;
            }
            OpCode::MakeStruct(type_name, field_count) => {
                let struct_def = self
                    .structs
                    .get(&type_name)
                    .cloned()
                    .ok_or_else(|| TinyLangError::runtime(format!("Struct '{type_name}' not defined")))?;
                if struct_def.fields.len() != field_count {
                    return Err(TinyLangError::runtime(format!(
                        "Struct '{}' expects {} fields, got {}",
                        type_name,
                        struct_def.fields.len(),
                        field_count
                    )));
                }

                let mut values = Vec::with_capacity(field_count);
                for _ in 0..field_count {
                    values.push(self.pop_value()?);
                }
                values.reverse();

                let mut fields = HashMap::new();
                for ((field_name, annotation), value) in struct_def.fields.iter().zip(values.into_iter()) {
                    if let Some(annotation) = annotation {
                        self.ensure_type_matches(annotation, &value)?;
                    }
                    fields.insert(field_name.clone(), value);
                }

                self.stack.push(struct_instance_value(&mut self.heap, type_name, fields));
            }
            OpCode::MakeEnumVariant(enum_name, variant_name, field_names) => {
                let mut values: Vec<Value> = (0..field_names.len())
                    .map(|_| self.pop_value())
                    .collect::<Result<_>>()?;
                values.reverse();
                let mut fields = HashMap::new();
                for (name, value) in field_names.iter().zip(values.into_iter()) {
                    fields.insert(name.clone(), value);
                }
                self.stack
                    .push(enum_variant_value(&mut self.heap, enum_name, variant_name, fields));
            }
            OpCode::CheckEnumVariant(enum_name, variant_name) => {
                let value = self.pop_value()?;
                let matches = match &value {
                    Value::EnumVariant(reference) => self.heap.with_enum_variant(reference, |ev| {
                        ev.enum_name == enum_name && ev.variant_name == variant_name
                    }),
                    _ => false,
                };
                self.stack.push(Value::Bool(matches));
            }
            OpCode::GetEnumField(field_name) => {
                let value = self.pop_value()?;
                match value {
                    Value::EnumVariant(reference) => {
                        let field_value = self.heap.with_enum_variant(&reference, |ev| {
                            ev.fields.get(&field_name).cloned().unwrap_or(Value::Null)
                        });
                        self.stack.push(field_value);
                    }
                    other => {
                        return Err(TinyLangError::runtime(format!(
                            "GetEnumField expects EnumVariant, got {}",
                            other.type_name_for_error()
                        )))
                    }
                }
            }
            OpCode::RuntimeError(message) => return Err(TinyLangError::runtime(message)),
            OpCode::Halt => {
                // 目前 frame 的程式碼已執行完畢（可能是頂層 script 或 import 子腳本）。
                // 彈出 frame 並恢復目錄；若已無其他 frame，則終止 VM。
                if let Some(frame) = self.frames.pop() {
                    if let Some(restore_dir) = self.frame_restore_dirs.pop().flatten() {
                        self.current_dir = restore_dir;
                    }
                    self.close_upvalues_from(frame.slot_offset);
                    // import 子腳本結束後把 stack 回到進入前的高度
                    self.stack.truncate(frame.slot_offset);
                }
                if self.frames.is_empty() {
                    return Ok(Some(Value::Null));
                }
                // 還有 caller frame（例如執行 import 後的主腳本），繼續執行
            }
        }

        Ok(None)
    }

    fn current_frame(&self) -> &CallFrame {
        self.frames.last().expect("vm must have an active frame")
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().expect("vm must have an active frame")
    }

    fn current_chunk(&self) -> &Chunk {
        &self.current_frame().function.chunk
    }

    fn read_local(&self, slot: usize) -> Result<Value> {
        let index = self.current_frame().slot_offset + slot;
        if let Some(cell) = self.open_upvalues.get(&index) {
            return Ok(cell.borrow().clone());
        }
        self.stack
            .get(index)
            .cloned()
            .ok_or_else(|| TinyLangError::runtime(format!("Local slot {slot} out of bounds")))
    }

    fn write_local(&mut self, slot: usize, value: Value) -> Result<()> {
        let index = self.current_frame().slot_offset + slot;
        if index >= self.stack.len() {
            return Err(TinyLangError::runtime(format!("Local slot {slot} out of bounds")));
        }
        self.stack[index] = value.clone();
        if let Some(cell) = self.open_upvalues.get(&index) {
            *cell.borrow_mut() = value;
        }
        Ok(())
    }

    fn capture_cells(&mut self, function: &Rc<CompiledFunction>) -> Result<Vec<Rc<RefCell<Value>>>> {
        let mut captured = Vec::with_capacity(function.capture_sources.len());
        for source in &function.capture_sources {
            match source {
                CaptureSource::Local(slot) => captured.push(self.capture_local_cell(*slot)?),
                CaptureSource::Upvalue(index) => {
                    let cell = self
                        .current_frame()
                        .upvalues
                        .get(*index)
                        .ok_or_else(|| TinyLangError::runtime(format!("Upvalue index {index} out of bounds")))?
                        .clone();
                    captured.push(cell);
                }
            }
        }
        Ok(captured)
    }

    fn capture_local_cell(&mut self, slot: usize) -> Result<Rc<RefCell<Value>>> {
        let index = self.current_frame().slot_offset + slot;
        if let Some(cell) = self.open_upvalues.get(&index) {
            return Ok(cell.clone());
        }
        let value = self
            .stack
            .get(index)
            .cloned()
            .ok_or_else(|| TinyLangError::runtime(format!("Local slot {slot} out of bounds")))?;
        let cell = Rc::new(RefCell::new(value));
        self.open_upvalues.insert(index, cell.clone());
        Ok(cell)
    }

    fn close_top_upvalue(&mut self) -> Result<()> {
        let top_index = self
            .stack
            .len()
            .checked_sub(1)
            .ok_or_else(|| TinyLangError::runtime("VM stack underflow"))?;
        if let Some(cell) = self.open_upvalues.remove(&top_index) {
            *cell.borrow_mut() = self.stack[top_index].clone();
        }
        self.stack.pop();
        Ok(())
    }

    fn close_upvalues_from(&mut self, start: usize) {
        let mut slots: Vec<usize> = self
            .open_upvalues
            .keys()
            .copied()
            .filter(|slot| *slot >= start)
            .collect();
        slots.sort_unstable();
        for slot in slots {
            if let Some(cell) = self.open_upvalues.remove(&slot) {
                if let Some(value) = self.stack.get(slot).cloned() {
                    *cell.borrow_mut() = value;
                }
            }
        }
    }

    fn handle_return(&mut self) -> Result<Value> {
        let return_value = self.pop_value().unwrap_or(Value::Null);
        let frame = self.frames.pop().expect("return requires active frame");
        let restore_dir = self.frame_restore_dirs.pop().flatten();
        if let Some(annotation) = &frame.function.return_type {
            self.ensure_type_matches(annotation, &return_value)?;
        }

        // 中文註解：函式返回前要先把仍開啟的 upvalue 關閉，讓 closure 繼續共享最後值。
        self.close_upvalues_from(frame.slot_offset);

        let callee_slot = if frame.function.takes_self {
            frame.slot_offset
        } else {
            frame.slot_offset.saturating_sub(1)
        };
        self.stack.truncate(callee_slot);

        if let Some(dir) = restore_dir {
            self.current_dir = dir;
        }

        if self.frames.is_empty() {
            return Ok(return_value);
        }

        self.stack.push(return_value.clone());
        Ok(return_value)
    }

    fn handle_runtime_error(&mut self, err: TinyLangError) -> Result<bool> {
        let Some(frame) = self.try_stack.pop() else {
            return Ok(false);
        };

        // 中文註解：回滾呼叫堆疊與 value stack，讓 catch block 在一致狀態下恢復執行。
        while self.frames.len() > frame.frame_depth {
            if let Some(unwound) = self.frames.pop() {
                if let Some(dir) = self.frame_restore_dirs.pop().flatten() {
                    self.current_dir = dir;
                }
                self.close_upvalues_from(unwound.slot_offset);
            }
        }
        self.close_upvalues_from(frame.stack_depth);
        self.stack.truncate(frame.stack_depth);

        let message = string_value(&mut self.heap, err.to_string());
        self.stack.push(message);

        if let Some(current) = self.frames.last_mut() {
            current.ip = frame.catch_ip;
            return Ok(true);
        }

        Ok(false)
    }

    fn execute_import(&mut self, path: &str) -> Result<()> {
        let candidate = self.current_dir.join(path);
        let canonical = std::fs::canonicalize(&candidate)
            .map_err(|_| TinyLangError::runtime(format!("Import file not found: {path}")))?;

        if self.imported_files.contains(&canonical) {
            return Ok(());
        }

        let source =
            std::fs::read_to_string(&canonical).map_err(|err| TinyLangError::io(err.to_string()))?;
        let program = crate::parse_source(&source)?;
        let chunk = crate::compiler::Compiler::compile_program_with_heap(&program, self.heap.clone())?;
        self.heap = chunk.heap.clone();

        self.imported_files.insert(canonical.clone());
        let previous_dir = self.current_dir.clone();
        if let Some(parent) = canonical.parent() {
            self.current_dir = parent.to_path_buf();
        }

        // 中文註解：import 腳本在同一個 VM 中跑，所以宣告的 globals 會自然留在目前環境。
        self.structs.extend(chunk.structs.clone());
        for (name, methods) in &chunk.methods {
            self.methods
                .entry(name.clone())
                .or_default()
                .extend(methods.clone());
        }
        self.push_script_frame(chunk, "<import>", Some(previous_dir));
        Ok(())
    }

    fn pop_value(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| TinyLangError::runtime("VM stack underflow"))
    }

    fn peek_value(&self) -> Result<&Value> {
        self.stack
            .last()
            .ok_or_else(|| TinyLangError::runtime("VM stack is empty"))
    }

    fn execute_add(&mut self) -> Result<()> {
        let right = self.pop_value()?;
        let left = self.pop_value()?;
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => self.stack.push(Value::Int(a + b)),
            (Value::String(a), Value::String(b)) => {
                let sa = self.heap.get_string(&a);
                let sb = self.heap.get_string(&b);
                let combined = sa + &sb;
                let result = string_value(&mut self.heap, combined);
                self.stack.push(result);
            }
            (a, b) => {
                return Err(TinyLangError::runtime(format!(
                    "Cannot add {} and {}",
                    a.type_name(),
                    b.type_name()
                )))
            }
        }
        Ok(())
    }

    fn execute_int_binary<F>(&mut self, action: &str, f: F) -> Result<()>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let (a, b) = self.pop_int_pair(action)?;
        self.stack.push(Value::Int(f(a, b)));
        Ok(())
    }

    fn execute_int_compare<F>(&mut self, op_name: &str, f: F) -> Result<()>
    where
        F: FnOnce(i64, i64) -> bool,
    {
        let (a, b) = self.pop_int_pair(&format!("compare with '{op_name}'"))?;
        self.stack.push(Value::Bool(f(a, b)));
        Ok(())
    }

    fn pop_int_pair(&mut self, action: &str) -> Result<(i64, i64)> {
        let right = self.pop_value()?;
        let left = self.pop_value()?;
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok((a, b)),
            (a, b) => Err(TinyLangError::runtime(format!(
                "Cannot {action} {} and {}",
                a.type_name(),
                b.type_name()
            ))),
        }
    }

    fn execute_call(&mut self, arg_count: usize) -> Result<()> {
        if self.stack.len() < arg_count + 1 {
            return Err(TinyLangError::runtime("VM stack underflow during call"));
        }

        let callee_index = self.stack.len() - arg_count - 1;
        let callee = self.stack[callee_index].clone();
        match callee {
            Value::CompiledFunction(function) => {
                if function.takes_self {
                    return Err(TinyLangError::runtime("method requires receiver"));
                }
                self.prepare_function_call(function, callee_index + 1, arg_count)?;
            }
            Value::Closure(closure_ref) => {
                // 呼叫閉包：取得函式和 upvalue，建立帶 upvalue 的 frame
                let closure = self.heap.get_closure(&closure_ref);
                if closure.function.takes_self {
                    return Err(TinyLangError::runtime("closure method requires receiver"));
                }
                let upvalues = closure.upvalues.clone();
                let function = closure.function.clone();
                self.prepare_closure_call(function, upvalues, callee_index + 1, arg_count)?;
            }
            Value::NativeFunction(native) => {
                self.call_native(native, callee_index, arg_count)?;
            }
            Value::VmBoundMethod(method) => {
                self.stack[callee_index] = Value::StructInstance(method.receiver.clone());
                self.prepare_function_call(method.method, callee_index, arg_count)?;
            }
            other => {
                return Err(TinyLangError::runtime(format!(
                    "value of type {} is not callable",
                    other.type_name_for_error()
                )))
            }
        }
        Ok(())
    }

    fn prepare_function_call(
        &mut self,
        function: Rc<CompiledFunction>,
        slot_offset: usize,
        arg_count: usize,
    ) -> Result<()> {
        if function.arity() != arg_count {
            let name = function.name.as_deref().unwrap_or("<fn>");
            return Err(TinyLangError::runtime(format!(
                "Function '{name}' expects {} arguments, got {}",
                function.arity(),
                arg_count
            )));
        }

        let args_start = if function.takes_self { slot_offset + 1 } else { slot_offset };
        for (index, (_, annotation)) in function.params.iter().enumerate() {
            if let Some(annotation) = annotation {
                let value = self
                    .stack
                    .get(args_start + index)
                    .cloned()
                    .ok_or_else(|| TinyLangError::runtime("VM argument slot missing"))?;
                self.ensure_type_matches(annotation, &value)?;
            }
        }

        self.frames.push(CallFrame {
            function,
            ip: 0,
            slot_offset,
            upvalues: Vec::new(), // 普通函式沒有 upvalue
        });
        self.frame_restore_dirs.push(None);
        Ok(())
    }

    /// 建立帶 upvalue 的閉包呼叫 frame。
    fn prepare_closure_call(
        &mut self,
        function: Rc<CompiledFunction>,
        upvalues: Vec<Rc<RefCell<Value>>>,
        slot_offset: usize,
        arg_count: usize,
    ) -> Result<()> {
        if function.arity() != arg_count {
            let name = function.name.as_deref().unwrap_or("<closure>");
            return Err(TinyLangError::runtime(format!(
                "Closure '{name}' expects {} arguments, got {}",
                function.arity(),
                arg_count
            )));
        }

        // 型別檢查參數
        for (index, (_, annotation)) in function.params.iter().enumerate() {
            if let Some(annotation) = annotation {
                let value = self
                    .stack
                    .get(slot_offset + index)
                    .cloned()
                    .ok_or_else(|| TinyLangError::runtime("VM argument slot missing"))?;
                self.ensure_type_matches(annotation, &value)?;
            }
        }

        self.frames.push(CallFrame {
            function,
            ip: 0,
            slot_offset,
            upvalues, // 傳入閉包的 upvalue 列表
        });
        self.frame_restore_dirs.push(None);
        Ok(())
    }

    fn call_native(&mut self, native: NativeFunction, callee_index: usize, arg_count: usize) -> Result<()> {
        if native.arity() != arg_count {
            return Err(TinyLangError::runtime(format!(
                "Function '{}' expects {} arguments, got {}",
                native.name(),
                native.arity(),
                arg_count
            )));
        }

        let mut args = self.stack.split_off(callee_index + 1);
        self.stack.pop();
        let result = self.execute_native(native, &mut args)?;
        self.stack.push(result);
        Ok(())
    }

    fn execute_native(&mut self, native: NativeFunction, args: &mut [Value]) -> Result<Value> {
        match native {
            NativeFunction::Len => match &args[0] {
                Value::Array(items) => {
                    let items = items.clone();
                    Ok(Value::Int(self.heap.with_array(&items, |arr| arr.len()) as i64))
                }
                Value::String(text) => {
                    let text = text.clone();
                    Ok(Value::Int(self.heap.get_string(&text).chars().count() as i64))
                }
                Value::Map(items) => {
                    let items = items.clone();
                    Ok(Value::Int(self.heap.with_map(&items, |m| m.len()) as i64))
                }
                other => Err(TinyLangError::runtime(format!(
                    "Function 'len' expects Array, String, or Map, got {}",
                    other.type_name_for_error()
                ))),
            },
            NativeFunction::Push => match &args[0] {
                Value::Array(items) => {
                    let items = items.clone();
                    let item = args[1].clone();
                    self.heap.with_array_mut(&items, |arr| arr.push(item));
                    Ok(Value::Null)
                }
                other => Err(TinyLangError::runtime(format!(
                    "Function 'push' expects Array as first argument, got {}",
                    other.type_name_for_error()
                ))),
            },
            NativeFunction::Pop => match &args[0] {
                Value::Array(items) => {
                    let items = items.clone();
                    Ok(self.heap.with_array_mut(&items, |arr| arr.pop().unwrap_or(Value::Null)))
                }
                other => Err(TinyLangError::runtime(format!(
                    "Function 'pop' expects Array, got {}",
                    other.type_name_for_error()
                ))),
            },
            NativeFunction::Str => {
                let rendered = render_value(&self.heap, &args[0]);
                Ok(string_value(&mut self.heap, rendered))
            }
            NativeFunction::Int => self.cast_to_int(&args[0]),
            NativeFunction::TypeOf => {
                let type_name = args[0].type_name_for_builtin();
                Ok(string_value(&mut self.heap, type_name))
            }
            NativeFunction::Range => {
                let start = self.expect_int(args[0].clone(), "range start")?;
                let end = self.expect_int(args[1].clone(), "range end")?;
                let items = (start..end).map(Value::Int).collect::<Vec<_>>();
                Ok(array_value(&mut self.heap, items))
            }
        }
    }

    fn read_index(&mut self, target: Value, index: Value) -> Result<Value> {
        match target {
            Value::Array(items) => {
                let idx = self.expect_index(index)?;
                self.heap.with_array(&items, |arr| {
                    arr.get(idx).cloned().ok_or_else(|| {
                        TinyLangError::runtime(format!(
                            "Index out of bounds: array length is {}, index is {}",
                            arr.len(),
                            idx
                        ))
                    })
                })
            }
            Value::String(text) => {
                let idx = self.expect_index(index)?;
                let s = self.heap.get_string(&text);
                let chars: Vec<char> = s.chars().collect();
                match chars.get(idx) {
                    Some(ch) => {
                        let ch_str = ch.to_string();
                        Ok(string_value(&mut self.heap, ch_str))
                    }
                    None => Err(TinyLangError::runtime(format!(
                        "Index out of bounds: string length is {}, index is {}",
                        chars.len(),
                        idx
                    ))),
                }
            }
            Value::Map(items) => {
                let key = self.expect_map_key(index)?;
                Ok(self.heap.with_map(&items, |m| m.get(&key).cloned().unwrap_or(Value::Null)))
            }
            other => Err(TinyLangError::runtime(format!(
                "Index access expects Array, String, or Map, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn assign_index(&mut self, target: Value, index: Value, value: Value) -> Result<()> {
        match target {
            Value::Array(items) => {
                let idx = self.expect_index(index)?;
                self.heap.with_array_mut(&items, |arr| {
                    if idx >= arr.len() {
                        Err(TinyLangError::runtime(format!(
                            "Index out of bounds: array length is {}, index is {}",
                            arr.len(),
                            idx
                        )))
                    } else {
                        arr[idx] = value;
                        Ok(())
                    }
                })
            }
            Value::Map(items) => {
                let key = self.expect_map_key(index)?;
                self.heap.with_map_mut(&items, |m| {
                    m.insert(key, value);
                });
                Ok(())
            }
            other => Err(TinyLangError::runtime(format!(
                "Index assignment expects Array or Map, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn read_field_or_method(&self, object: Value, field: &str) -> Result<Value> {
        match object {
            Value::StructInstance(instance) => {
                let type_name = self.heap.with_struct_instance(&instance, |inst| inst.type_name.clone());
                let field_value = self.heap.with_struct_instance(&instance, |inst| inst.fields.get(field).cloned());

                if let Some(value) = field_value {
                    return Ok(value);
                }

                let method = self
                    .methods
                    .get(&type_name)
                    .and_then(|methods| methods.get(field))
                    .cloned()
                    .ok_or_else(|| {
                        TinyLangError::runtime(format!(
                            "Struct '{}' has no field or method '{}'",
                            type_name, field
                        ))
                    })?;
                Ok(Value::VmBoundMethod(VmBoundMethodValue {
                    receiver: instance,
                    method,
                }))
            }
            // enum variant 也支援欄位存取（例如繫結後存取 variant.field_name）
            Value::EnumVariant(reference) => {
                let field_value = self.heap.with_enum_variant(&reference, |ev| ev.fields.get(field).cloned());
                field_value.ok_or_else(|| {
                    let variant_name = self.heap.get_enum_variant(&reference).variant_name.clone();
                    TinyLangError::runtime(format!(
                        "Enum variant '{}' has no field '{}'",
                        variant_name, field
                    ))
                })
            }
            other => Err(TinyLangError::runtime(format!(
                "Field access expects struct instance, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn assign_field(&mut self, object: Value, field: &str, value: Value) -> Result<()> {
        match object {
            Value::StructInstance(instance) => {
                let type_name = self.heap.with_struct_instance(&instance, |inst| inst.type_name.clone());
                let struct_def = self
                    .structs
                    .get(&type_name)
                    .ok_or_else(|| TinyLangError::runtime(format!("Struct '{}' not defined", type_name)))?
                    .clone();
                let field_def = struct_def
                    .fields
                    .iter()
                    .find(|(name, _)| name == field)
                    .ok_or_else(|| {
                        TinyLangError::runtime(format!(
                            "Struct '{}' has no field '{}'",
                            type_name, field
                        ))
                    })?
                    .clone();
                if let Some(annotation) = &field_def.1 {
                    self.ensure_type_matches(annotation, &value)?;
                }
                self.heap.with_struct_instance_mut(&instance, |inst| {
                    inst.fields.insert(field.to_string(), value);
                });
                Ok(())
            }
            other => Err(TinyLangError::runtime(format!(
                "Field assignment expects struct instance, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn ensure_type_matches(&self, annotation: &TypeAnnotation, value: &Value) -> Result<()> {
        if self.value_matches_type(annotation, value) {
            Ok(())
        } else {
            Err(TinyLangError::runtime(format!(
                "Expected {}, got {}",
                annotation.display_name(),
                value.type_name_for_error()
            )))
        }
    }

    fn value_matches_type(&self, annotation: &TypeAnnotation, value: &Value) -> bool {
        match annotation {
            TypeAnnotation::Any => true,
            TypeAnnotation::Int => matches!(value, Value::Int(_)),
            TypeAnnotation::Str => matches!(value, Value::String(_)),
            TypeAnnotation::Bool => matches!(value, Value::Bool(_)),
            TypeAnnotation::ArrayOf(inner) => match value {
                Value::Array(items) => {
                    let items = items.clone();
                    self.heap.with_array(&items, |arr| {
                        arr.iter().all(|item| self.value_matches_type(inner, item))
                    })
                }
                _ => false,
            },
            TypeAnnotation::MapOf(inner) => match value {
                Value::Map(items) => {
                    let items = items.clone();
                    self.heap.with_map(&items, |m| {
                        m.values().all(|item| self.value_matches_type(inner, item))
                    })
                }
                _ => false,
            },
            TypeAnnotation::Named(name) => match value {
                Value::StructInstance(instance) => {
                    let instance = instance.clone();
                    self.heap.with_struct_instance(&instance, |inst| &inst.type_name == name)
                }
                _ => false,
            },
        }
    }

    fn values_equal(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::String(a), Value::String(b)) => {
                self.heap.get_string(a) == self.heap.get_string(b)
            }
            _ => left == right,
        }
    }

    fn expect_index(&self, value: Value) -> Result<usize> {
        match value {
            Value::Int(index) if index >= 0 => Ok(index as usize),
            Value::Int(index) => Err(TinyLangError::runtime(format!(
                "Index must be non-negative, got {}",
                index
            ))),
            other => Err(TinyLangError::runtime(format!(
                "Index must be Int, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn expect_map_key(&self, value: Value) -> Result<String> {
        match value {
            Value::String(text) => Ok(self.heap.get_string(&text)),
            other => Err(TinyLangError::runtime(format!(
                "Map key must be String, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn expect_int(&self, value: Value, label: &str) -> Result<i64> {
        match value {
            Value::Int(number) => Ok(number),
            other => Err(TinyLangError::runtime(format!(
                "{label} must be Int, got {}",
                other.type_name_for_error()
            ))),
        }
    }

    fn cast_to_int(&self, value: &Value) -> Result<Value> {
        match value {
            Value::Int(number) => Ok(Value::Int(*number)),
            Value::Bool(flag) => Ok(Value::Int(if *flag { 1 } else { 0 })),
            Value::String(text) => {
                let s = self.heap.get_string(text);
                s.parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| TinyLangError::runtime(format!("Cannot convert String '{}' to Int", s)))
            }
            other => Err(TinyLangError::runtime(format!(
                "Cannot convert {} to Int",
                other.type_name_for_error()
            ))),
        }
    }

    /// 當 heap 超過閾值時，自動觸發 mark-and-sweep GC。
    ///
    /// 收集 stack、globals 和所有 frame upvalue 作為根。
    fn try_collect_garbage(&mut self) {
        if !self.heap.should_collect() {
            return;
        }
        // 收集所有可達的根值
        let mut roots: Vec<Value> = self.stack.clone();
        roots.extend(self.globals.values().cloned());
        // 包含所有 frame 的 upvalue cell 中的值
        for frame in &self.frames {
            for cell in &frame.upvalues {
                let v: Value = cell.borrow().clone();
                roots.push(v);
            }
        }
        // 收集所有 frame 的常數池（字串常數在 heap 上）
        let constant_roots: Vec<Value> = self
            .frames
            .iter()
            .flat_map(|f| f.function.chunk.constants.iter().cloned())
            .collect();
        self.heap.mark_and_sweep(&roots, &constant_roots);
    }

    fn install_natives(&mut self) {
        self.globals
            .insert("len".into(), Value::NativeFunction(NativeFunction::Len));
        self.globals
            .insert("push".into(), Value::NativeFunction(NativeFunction::Push));
        self.globals
            .insert("pop".into(), Value::NativeFunction(NativeFunction::Pop));
        self.globals
            .insert("str".into(), Value::NativeFunction(NativeFunction::Str));
        self.globals
            .insert("int".into(), Value::NativeFunction(NativeFunction::Int));
        self.globals
            .insert("type_of".into(), Value::NativeFunction(NativeFunction::TypeOf));
        self.globals
            .insert("typeof".into(), Value::NativeFunction(NativeFunction::TypeOf));
        self.globals
            .insert("range".into(), Value::NativeFunction(NativeFunction::Range));
    }
}

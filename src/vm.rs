//! Bytecode VM。
//!
//! VM 會執行 compiler 產生的 Chunk，並維護 stack 與 call frame。
//! Phase 7 新增：閉包 upvalue 支援、enum 指令、自動 GC 觸發。

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Stdout, Write};
use std::rc::Rc;

use crate::ast::TypeAnnotation;
use crate::compiler::{Chunk, OpCode};
use crate::environment::{
    array_value, closure_value, enum_variant_value, map_value, render_value, string_value,
    struct_instance_value, CompiledFunction, NativeFunction, StructDef, Value, VmBoundMethodValue,
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

/// Bytecode 虛擬機。
pub struct VM<W: Write> {
    pub stack: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, Value>,
    structs: HashMap<String, StructDef>,
    methods: HashMap<String, HashMap<String, Rc<CompiledFunction>>>,
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
            output,
            heap: GcHeap::new(),
        };
        vm.install_natives();
        vm
    }

    /// 執行整個 chunk。
    pub fn run(&mut self, mut chunk: Chunk) -> Result<Value> {
        self.stack.clear();
        self.frames.clear();
        self.structs = chunk.structs.clone();
        self.methods = chunk.methods.clone();

        // Adopt the compiler's heap so string constants remain valid.
        // Replace the chunk's heap with a fresh one so the Rc<Chunk> can still be formed.
        let compiler_heap = std::mem::replace(&mut chunk.heap, GcHeap::new());
        self.heap = compiler_heap;

        let script = Rc::new(CompiledFunction {
            name: Some("<script>".into()),
            params: Vec::new(),
            return_type: None,
            chunk: Rc::new(chunk),
            local_count: 0,
            takes_self: false,
            capture_names: Vec::new(),
        });

        self.frames.push(CallFrame {
            function: script,
            ip: 0,
            slot_offset: 0,
            upvalues: Vec::new(), // 腳本頂層沒有 upvalue
        });

        loop {
            let instruction = {
                let Some(frame) = self.frames.last_mut() else {
                    return Ok(Value::Null);
                };

                if frame.ip >= frame.function.chunk.code.len() {
                    OpCode::Halt
                } else {
                    let opcode = frame.function.chunk.code[frame.ip].clone();
                    frame.ip += 1;
                    opcode
                }
            };

            match instruction {
                OpCode::Constant(index) => {
                    let value = self.current_chunk().constants[index].clone();
                    self.stack.push(value);
                }
                OpCode::Pop => {
                    self.pop_value()?;
                }
                OpCode::GetLocal(slot) => {
                    let index = self.current_frame().slot_offset + slot;
                    let value = self
                        .stack
                        .get(index)
                        .cloned()
                        .ok_or_else(|| TinyLangError::runtime(format!("Local slot {slot} out of bounds")))?;
                    self.stack.push(value);
                }
                OpCode::SetLocal(slot) => {
                    let value = self.pop_value()?;
                    let index = self.current_frame().slot_offset + slot;
                    if index >= self.stack.len() {
                        return Err(TinyLangError::runtime(format!("Local slot {slot} out of bounds")));
                    }
                    self.stack[index] = value;
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
                    // 從當前 frame 的 upvalue cell 讀取值
                    let value = self
                        .frames
                        .last()
                        .expect("active frame")
                        .upvalues
                        .get(index)
                        .ok_or_else(|| TinyLangError::runtime(format!("Upvalue index {index} out of bounds")))?
                        .borrow()
                        .clone();
                    self.stack.push(value);
                }
                OpCode::SetUpvalue(index) => {
                    // 寫入當前 frame 的 upvalue cell
                    let value = self.pop_value()?;
                    let cell = self
                        .frames
                        .last()
                        .expect("active frame")
                        .upvalues
                        .get(index)
                        .ok_or_else(|| TinyLangError::runtime(format!("Upvalue index {index} out of bounds")))?
                        .clone(); // clone Rc（cheap），避免 borrow 衝突
                    *cell.borrow_mut() = value;
                }
                OpCode::CloseUpvalue => {
                    // 在此實作中，upvalue 在建立閉包時已複製到 Rc<RefCell<>>
                    // CloseUpvalue 只需彈出 stack 頂部值（與 Pop 相同）
                    self.pop_value()?;
                }
                OpCode::MakeClosure(const_idx) => {
                    // 從常數池取得函式原型
                    let function = match &self.current_chunk().constants[const_idx] {
                        Value::CompiledFunction(f) => f.clone(),
                        _ => return Err(TinyLangError::runtime("MakeClosure: expected CompiledFunction constant")),
                    };
                    let capture_count = function.capture_names.len();
                    // 從 stack 取出捕獲值（後進先出，需反轉恢復原始順序）
                    let mut captured: Vec<Rc<RefCell<Value>>> = (0..capture_count)
                        .map(|_| self.pop_value().map(|v| Rc::new(RefCell::new(v))))
                        .collect::<Result<_>>()?;
                    captured.reverse();
                    // 建立 closure 物件並推入 stack
                    let closure = closure_value(&mut self.heap, function, captured);
                    self.stack.push(closure);
                    // 分配後檢查是否需要觸發 GC
                    self.try_collect_garbage();
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
                OpCode::Jump(target) => {
                    self.current_frame_mut().ip = target;
                }
                OpCode::JumpIfFalse(target) => {
                    if !self.peek_value()?.is_truthy() {
                        self.current_frame_mut().ip = target;
                    }
                }
                OpCode::Loop(target) => {
                    self.current_frame_mut().ip = target;
                }
                OpCode::Call(arg_count) => self.execute_call(arg_count)?,
                OpCode::Return => {
                    let return_value = self.pop_value().unwrap_or(Value::Null);
                    let frame = self.frames.pop().expect("return requires active frame");
                    if let Some(annotation) = &frame.function.return_type {
                        self.ensure_type_matches(annotation, &return_value)?;
                    }

                    let callee_slot = if frame.function.takes_self {
                        frame.slot_offset
                    } else {
                        frame.slot_offset.saturating_sub(1)
                    };
                    self.stack.truncate(callee_slot);

                    if self.frames.is_empty() {
                        return Ok(return_value);
                    }

                    self.stack.push(return_value);
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
                    let arr = array_value(&mut self.heap, items);
                    self.stack.push(arr);
                    // 陣列分配後檢查是否需要觸發 GC
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
                    let map_val = map_value(&mut self.heap, map);
                    self.stack.push(map_val);
                    // Map 分配後檢查是否需要觸發 GC
                    self.try_collect_garbage();
                }
                OpCode::Index => {
                    let index = self.pop_value()?;
                    let target = self.pop_value()?;
                    let value = self.read_index(target, index)?;
                    self.stack.push(value);
                }
                OpCode::SetIndex => {
                    let value = self.pop_value()?;
                    let index = self.pop_value()?;
                    let target = self.pop_value()?;
                    self.assign_index(target, index, value)?;
                }
                OpCode::GetField(field) => {
                    let object = self.pop_value()?;
                    let value = self.read_field_or_method(object, &field)?;
                    self.stack.push(value);
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

                    let instance = struct_instance_value(&mut self.heap, type_name, fields);
                    self.stack.push(instance);
                }
                OpCode::MakeEnumVariant(enum_name, variant_name, field_names) => {
                    // 從 stack 彈出欄位值（後進先出，需反轉）
                    let mut values: Vec<Value> = (0..field_names.len())
                        .map(|_| self.pop_value())
                        .collect::<Result<_>>()?;
                    values.reverse();
                    // 建立欄位 HashMap
                    let mut fields = HashMap::new();
                    for (name, value) in field_names.iter().zip(values.into_iter()) {
                        fields.insert(name.clone(), value);
                    }
                    // 建立 enum variant 值並推入 stack
                    let variant = enum_variant_value(&mut self.heap, enum_name, variant_name, fields);
                    self.stack.push(variant);
                }
                OpCode::CheckEnumVariant(enum_name, variant_name) => {
                    // 彈出頂部值，檢查是否為指定的 enum variant，推入 bool
                    let value = self.pop_value()?;
                    let matches = match &value {
                        Value::EnumVariant(reference) => {
                            self.heap.with_enum_variant(reference, |ev| {
                                ev.enum_name == enum_name && ev.variant_name == variant_name
                            })
                        }
                        _ => false,
                    };
                    self.stack.push(Value::Bool(matches));
                }
                OpCode::GetEnumField(field_name) => {
                    // 彈出 enum variant，推入指定欄位的值
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
                OpCode::Halt => return Ok(self.stack.last().cloned().unwrap_or(Value::Null)),
            }
        }
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
                roots.push(cell.borrow().clone());
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

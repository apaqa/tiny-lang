//! 執行環境與執行期值定義。
//!
//! tree-walking interpreter 與 bytecode VM 會共用大部分執行期值，
//! 這裡集中管理 Value、函式、struct 定義與詞法環境。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{Statement, TypeAnnotation};
use crate::compiler::Chunk;
use crate::error::{Result, TinyLangError};

/// Array 的共享引用。
pub type ArrayRef = Rc<RefCell<Vec<Value>>>;

/// Map 的共享引用。
pub type MapRef = Rc<RefCell<HashMap<String, Value>>>;

/// Struct instance 欄位表的共享引用。
pub type StructFieldsRef = Rc<RefCell<HashMap<String, Value>>>;

/// 執行期值。
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    String(String),
    Bool(bool),
    Array(ArrayRef),
    Map(MapRef),
    StructInstance(StructInstanceValue),
    Function(FunctionValue),
    BoundMethod(BoundMethodValue),
    CompiledFunction(Rc<CompiledFunction>),
    NativeFunction(NativeFunction),
    VmBoundMethod(VmBoundMethodValue),
    Builtin(BuiltinFunction),
    Null,
}

/// 變數綁定，包含值與可選型別註記。
#[derive(Debug, Clone)]
pub struct Binding {
    pub value: Value,
    pub type_annotation: Option<TypeAnnotation>,
}

/// tree-walking interpreter 使用的函式表示。
#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub name: Option<String>,
    pub params: Vec<(String, Option<TypeAnnotation>)>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Vec<Statement>,
    pub closure: EnvRef,
}

/// Struct 定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Option<TypeAnnotation>)>,
}

/// 執行期 struct instance。
#[derive(Debug, Clone)]
pub struct StructInstanceValue {
    pub type_name: String,
    pub fields: StructFieldsRef,
}

/// tree-walking interpreter 綁定 receiver 後的方法值。
#[derive(Debug, Clone)]
pub struct BoundMethodValue {
    pub receiver: StructInstanceValue,
    pub method: FunctionValue,
}

/// bytecode VM 使用的已編譯函式。
#[derive(Debug, Clone)]
pub struct CompiledFunction {
    pub name: Option<String>,
    pub params: Vec<(String, Option<TypeAnnotation>)>,
    pub return_type: Option<TypeAnnotation>,
    pub chunk: Rc<Chunk>,
    pub local_count: usize,
    pub takes_self: bool,
}

impl CompiledFunction {
    pub fn arity(&self) -> usize {
        self.params.len()
    }
}

/// VM 的原生函式值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFunction {
    Len,
    Push,
    Pop,
    Str,
    Int,
    TypeOf,
    Range,
}

/// VM 綁定 receiver 後的方法值。
#[derive(Debug, Clone)]
pub struct VmBoundMethodValue {
    pub receiver: StructInstanceValue,
    pub method: Rc<CompiledFunction>,
}

/// tree interpreter 的內建函式表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Len,
    Push,
    Pop,
    Str,
    Int,
    TypeOf,
    Input,
    Range,
    Keys,
    Values,
    Abs,
    Max,
    Min,
    Pow,
    Split,
    Join,
    Trim,
    Upper,
    Lower,
    Contains,
    Replace,
    Sort,
    Reverse,
    Map,
    Filter,
    Reduce,
    Find,
    Assert,
}

pub type EnvRef = Rc<RefCell<Environment>>;

/// 環境同時保存變數、struct 定義與 method 定義。
#[derive(Debug, Clone)]
pub struct Environment {
    values: HashMap<String, Binding>,
    structs: HashMap<String, StructDef>,
    methods: HashMap<String, HashMap<String, FunctionValue>>,
    parent: Option<EnvRef>,
}

impl Environment {
    pub fn new() -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            structs: HashMap::new(),
            methods: HashMap::new(),
            parent: None,
        }))
    }

    pub fn enclosed(parent: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            structs: HashMap::new(),
            methods: HashMap::new(),
            parent: Some(parent),
        }))
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.define_typed(name, value, None);
    }

    pub fn define_typed(&mut self, name: String, value: Value, type_annotation: Option<TypeAnnotation>) {
        self.values.insert(
            name,
            Binding {
                value,
                type_annotation,
            },
        );
    }

    pub fn get(&self, name: &str) -> Result<Value> {
        if let Some(binding) = self.values.get(name) {
            return Ok(binding.value.clone());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow().get(name);
        }

        Err(TinyLangError::runtime(format!("Variable '{name}' not defined")))
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        if let Some(binding) = self.values.get_mut(name) {
            binding.value = value;
            return Ok(());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }

        Err(TinyLangError::runtime(format!("Variable '{name}' not defined")))
    }

    pub fn get_annotation(&self, name: &str) -> Result<Option<TypeAnnotation>> {
        if let Some(binding) = self.values.get(name) {
            return Ok(binding.type_annotation.clone());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow().get_annotation(name);
        }

        Err(TinyLangError::runtime(format!("Variable '{name}' not defined")))
    }

    pub fn define_struct(&mut self, name: String, def: StructDef) {
        self.structs.insert(name, def);
    }

    pub fn get_struct(&self, name: &str) -> Result<StructDef> {
        if let Some(def) = self.structs.get(name) {
            return Ok(def.clone());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow().get_struct(name);
        }

        Err(TinyLangError::runtime(format!("Struct '{name}' not defined")))
    }

    pub fn define_method(&mut self, struct_name: String, method_name: String, method: FunctionValue) {
        self.methods
            .entry(struct_name)
            .or_default()
            .insert(method_name, method);
    }

    pub fn get_method(&self, struct_name: &str, method_name: &str) -> Result<FunctionValue> {
        if let Some(methods) = self.methods.get(struct_name) {
            if let Some(method) = methods.get(method_name) {
                return Ok(method.clone());
            }
        }

        if let Some(parent) = &self.parent {
            return parent.borrow().get_method(struct_name, method_name);
        }

        Err(TinyLangError::runtime(format!(
            "Method '{}.{}' not defined",
            struct_name, method_name
        )))
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(value) => *value,
            Value::Int(value) => *value != 0,
            Value::String(value) => !value.is_empty(),
            Value::Array(items) => !items.borrow().is_empty(),
            Value::Map(items) => !items.borrow().is_empty(),
            Value::StructInstance(_) => true,
            Value::Function(_)
            | Value::BoundMethod(_)
            | Value::CompiledFunction(_)
            | Value::NativeFunction(_)
            | Value::VmBoundMethod(_)
            | Value::Builtin(_) => true,
            Value::Null => false,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
            Value::StructInstance(_) => "Struct",
            Value::Function(_)
            | Value::BoundMethod(_)
            | Value::CompiledFunction(_)
            | Value::NativeFunction(_)
            | Value::VmBoundMethod(_)
            | Value::Builtin(_) => "Function",
            Value::Null => "Null",
        }
    }

    pub fn type_name_for_builtin(&self) -> String {
        match self {
            Value::Int(_) => "int".into(),
            Value::String(_) => "string".into(),
            Value::Bool(_) => "bool".into(),
            Value::Array(_) => "array".into(),
            Value::Map(_) => "map".into(),
            Value::StructInstance(instance) => instance.type_name.clone(),
            Value::Function(_)
            | Value::BoundMethod(_)
            | Value::CompiledFunction(_)
            | Value::NativeFunction(_)
            | Value::VmBoundMethod(_)
            | Value::Builtin(_) => "function".into(),
            Value::Null => "null".into(),
        }
    }

    /// 給錯誤訊息使用的型別名稱。
    pub fn type_name_for_error(&self) -> String {
        match self {
            Value::Int(_) => "int".into(),
            Value::String(_) => "str".into(),
            Value::Bool(_) => "bool".into(),
            Value::Array(items) => {
                let items = items.borrow();
                if let Some(first) = items.first() {
                    format!("[{}]", first.type_name_for_error())
                } else {
                    "[any]".into()
                }
            }
            Value::Map(items) => {
                let items = items.borrow();
                if let Some(first) = items.values().next() {
                    format!("{{{}}}", first.type_name_for_error())
                } else {
                    "{any}".into()
                }
            }
            Value::StructInstance(instance) => instance.type_name.clone(),
            Value::Function(_)
            | Value::BoundMethod(_)
            | Value::CompiledFunction(_)
            | Value::NativeFunction(_)
            | Value::VmBoundMethod(_)
            | Value::Builtin(_) => "function".into(),
            Value::Null => "null".into(),
        }
    }
}

impl PartialEq for FunctionValue {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.params == other.params
            && self.return_type == other.return_type
            && self.body == other.body
    }
}

impl Eq for FunctionValue {}

impl PartialEq for StructInstanceValue {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name && *self.fields.borrow() == *other.fields.borrow()
    }
}

impl Eq for StructInstanceValue {}

impl PartialEq for BoundMethodValue {
    fn eq(&self, other: &Self) -> bool {
        self.receiver == other.receiver && self.method == other.method
    }
}

impl Eq for BoundMethodValue {}

impl PartialEq for CompiledFunction {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.params == other.params
            && self.return_type == other.return_type
            && self.local_count == other.local_count
            && self.takes_self == other.takes_self
            && self.chunk == other.chunk
    }
}

impl Eq for CompiledFunction {}

impl PartialEq for VmBoundMethodValue {
    fn eq(&self, other: &Self) -> bool {
        self.receiver == other.receiver && self.method == other.method
    }
}

impl Eq for VmBoundMethodValue {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Array(a), Value::Array(b)) => *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            (Value::StructInstance(a), Value::StructInstance(b)) => a == b,
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::BoundMethod(a), Value::BoundMethod(b)) => a == b,
            (Value::CompiledFunction(a), Value::CompiledFunction(b)) => a == b,
            (Value::NativeFunction(a), Value::NativeFunction(b)) => a == b,
            (Value::VmBoundMethod(a), Value::VmBoundMethod(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Array(items) => {
                let rendered = items
                    .borrow()
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "[{rendered}]")
            }
            Value::Map(items) => {
                let mut entries = items
                    .borrow()
                    .iter()
                    .map(|(key, value)| format!("\"{key}\": {value}"))
                    .collect::<Vec<_>>();
                entries.sort();
                write!(f, "{{{}}}", entries.join(", "))
            }
            Value::StructInstance(instance) => {
                let mut entries = instance
                    .fields
                    .borrow()
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .collect::<Vec<_>>();
                entries.sort();
                write!(f, "{} {{ {} }}", instance.type_name, entries.join(", "))
            }
            Value::Function(function) => match &function.name {
                Some(name) => write!(f, "<fn {name}>"),
                None => write!(f, "<lambda>"),
            },
            Value::BoundMethod(method) => match &method.method.name {
                Some(name) => write!(f, "<bound {name}>"),
                None => write!(f, "<bound method>"),
            },
            Value::CompiledFunction(function) => match &function.name {
                Some(name) => write!(f, "<compiled {name}>"),
                None => write!(f, "<compiled lambda>"),
            },
            Value::NativeFunction(native) => write!(f, "<native {}>", native.name()),
            Value::VmBoundMethod(method) => match &method.method.name {
                Some(name) => write!(f, "<bound {name}>"),
                None => write!(f, "<bound vm method>"),
            },
            Value::Builtin(_) => write!(f, "<builtin>"),
            Value::Null => write!(f, "null"),
        }
    }
}

impl NativeFunction {
    pub fn name(&self) -> &'static str {
        match self {
            NativeFunction::Len => "len",
            NativeFunction::Push => "push",
            NativeFunction::Pop => "pop",
            NativeFunction::Str => "str",
            NativeFunction::Int => "int",
            NativeFunction::TypeOf => "type_of",
            NativeFunction::Range => "range",
        }
    }

    pub fn arity(&self) -> usize {
        match self {
            NativeFunction::Len => 1,
            NativeFunction::Push => 2,
            NativeFunction::Pop => 1,
            NativeFunction::Str => 1,
            NativeFunction::Int => 1,
            NativeFunction::TypeOf => 1,
            NativeFunction::Range => 2,
        }
    }
}

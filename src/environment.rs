//! 執行期值與環境定義。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{Statement, TypeAnnotation};
use crate::compiler::Chunk;
use crate::error::{Result, TinyLangError};
use crate::gc::{
    ClosureObject, EnumVariantObject, GcArrayRef, GcClosureRef, GcEnumVariantRef, GcHeap, GcMapRef, GcStringRef,
    GcStructRef, StructInstanceObject,
};

/// 執行期值。
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    String(GcStringRef),
    Bool(bool),
    Array(GcArrayRef),
    Map(GcMapRef),
    Closure(GcClosureRef),
    StructInstance(GcStructRef),
    EnumVariant(GcEnumVariantRef),
    Function(FunctionValue),
    BoundMethod(BoundMethodValue),
    CompiledFunction(Rc<CompiledFunction>),
    NativeFunction(NativeFunction),
    VmBoundMethod(VmBoundMethodValue),
    Builtin(BuiltinFunction),
    Null,
}

/// 變數綁定與型別註記。
#[derive(Debug, Clone)]
pub struct Binding {
    pub value: Value,
    pub type_annotation: Option<TypeAnnotation>,
}

/// tree-walking interpreter 使用的函式值。
#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub name: Option<String>,
    pub params: Vec<(String, Option<TypeAnnotation>)>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Vec<Statement>,
    pub closure: EnvRef,
}

/// struct 定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Option<TypeAnnotation>)>,
}

/// 與舊程式碼相容的 struct instance 快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructInstanceValue {
    pub type_name: String,
    pub fields: HashMap<String, Value>,
}

/// tree-walking interpreter 的 bound method。
#[derive(Debug, Clone)]
pub struct BoundMethodValue {
    pub receiver: GcStructRef,
    pub method: FunctionValue,
}

/// bytecode VM 使用的函式原型。
#[derive(Debug, Clone)]
/// 中文註解：closure 在建立時要知道捕獲來源，才能共享正確的 upvalue cell。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSource {
    Local(usize),
    Upvalue(usize),
}

#[derive(Debug, Clone)]
pub struct CompiledFunction {
    pub name: Option<String>,
    pub params: Vec<(String, Option<TypeAnnotation>)>,
    pub return_type: Option<TypeAnnotation>,
    pub chunk: Rc<Chunk>,
    pub local_count: usize,
    pub takes_self: bool,
    pub capture_names: Vec<String>,
    /// 中文註解：描述每個捕獲值是來自 local 還是外層 upvalue。
    pub capture_sources: Vec<CaptureSource>,
}

impl CompiledFunction {
    pub fn arity(&self) -> usize {
        self.params.len()
    }
}

/// VM 內建函式。
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

/// VM 的 bound method。
#[derive(Debug, Clone)]
pub struct VmBoundMethodValue {
    pub receiver: GcStructRef,
    pub method: Rc<CompiledFunction>,
}

/// tree interpreter 的內建函式。
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

/// enum variant 定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantDef {
    pub name: String,
    pub fields: Option<Vec<(String, Option<TypeAnnotation>)>>,
}

/// enum 定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariantDef>,
}

pub type EnvRef = Rc<RefCell<Environment>>;

/// 執行環境。
#[derive(Debug, Clone)]
pub struct Environment {
    values: HashMap<String, Binding>,
    structs: HashMap<String, StructDef>,
    enums: HashMap<String, EnumDef>,
    methods: HashMap<String, HashMap<String, FunctionValue>>,
    parent: Option<EnvRef>,
}

impl Environment {
    pub fn new() -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            methods: HashMap::new(),
            parent: None,
        }))
    }

    pub fn enclosed(parent: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
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

    pub fn define_enum(&mut self, name: String, def: EnumDef) {
        self.enums.insert(name, def);
    }

    pub fn get_enum(&self, name: &str) -> Result<EnumDef> {
        if let Some(def) = self.enums.get(name) {
            return Ok(def.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().get_enum(name);
        }
        Err(TinyLangError::runtime(format!("Enum '{name}' not defined")))
    }

    pub fn define_method(&mut self, struct_name: String, method_name: String, method: FunctionValue) {
        self.methods.entry(struct_name).or_default().insert(method_name, method);
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
            Value::Null => false,
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
            Value::Closure(_) => "Closure",
            Value::StructInstance(_) => "Struct",
            Value::EnumVariant(_) => "Enum",
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
            Value::Closure(_) => "function".into(),
            Value::StructInstance(_) => "struct".into(),
            Value::EnumVariant(_) => "enum".into(),
            Value::Function(_)
            | Value::BoundMethod(_)
            | Value::CompiledFunction(_)
            | Value::NativeFunction(_)
            | Value::VmBoundMethod(_)
            | Value::Builtin(_) => "function".into(),
            Value::Null => "null".into(),
        }
    }

    pub fn type_name_for_error(&self) -> String {
        match self {
            Value::Int(_) => "int".into(),
            Value::String(_) => "str".into(),
            Value::Bool(_) => "bool".into(),
            Value::Array(_) => "array".into(),
            Value::Map(_) => "map".into(),
            Value::Closure(_) => "function".into(),
            Value::StructInstance(_) => "struct".into(),
            Value::EnumVariant(_) => "enum".into(),
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
            && self.capture_names == other.capture_names
            && self.capture_sources == other.capture_sources
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
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => a == b,
            (Value::StructInstance(a), Value::StructInstance(b)) => a == b,
            (Value::EnumVariant(a), Value::EnumVariant(b)) => a == b,
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::BoundMethod(a), Value::BoundMethod(b)) => a == b,
            (Value::CompiledFunction(a), Value::CompiledFunction(b)) => a == b,
            (Value::NativeFunction(a), Value::NativeFunction(b)) => a == b,
            (Value::VmBoundMethod(a), Value::VmBoundMethod(b)) => a == b,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::String(_) => write!(f, "<string>"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Array(_) => write!(f, "<array>"),
            Value::Map(_) => write!(f, "<map>"),
            Value::Closure(_) => write!(f, "<closure>"),
            Value::StructInstance(_) => write!(f, "<struct>"),
            Value::EnumVariant(_) => write!(f, "<enum-variant>"),
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

/// 將值轉成可讀字串。
pub fn render_value(heap: &GcHeap, value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::String(reference) => heap.get_string(reference),
        Value::Bool(value) => value.to_string(),
        Value::Array(reference) => {
            let rendered = heap.with_array(reference, |items| {
                items
                    .iter()
                    .map(|item| render_value(heap, item))
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            format!("[{rendered}]")
        }
        Value::Map(reference) => {
            let mut entries = heap.with_map(reference, |items| {
                items.iter()
                    .map(|(key, value)| format!("\"{key}\": {}", render_value(heap, value)))
                    .collect::<Vec<_>>()
            });
            entries.sort();
            format!("{{{}}}", entries.join(", "))
        }
        Value::Closure(reference) => {
            let closure = heap.get_closure(reference);
            match &closure.function.name {
                Some(name) => format!("<closure {name}>"),
                None => "<closure>".into(),
            }
        }
        Value::StructInstance(reference) => {
            let instance = heap.get_struct_instance(reference);
            let mut entries = instance
                .fields
                .iter()
                .map(|(key, value)| format!("{key}: {}", render_value(heap, value)))
                .collect::<Vec<_>>();
            entries.sort();
            format!("{} {{ {} }}", instance.type_name, entries.join(", "))
        }
        Value::EnumVariant(reference) => {
            let variant = heap.get_enum_variant(reference);
            if variant.fields.is_empty() {
                format!("{}::{}", variant.enum_name, variant.variant_name)
            } else {
                let mut entries = variant
                    .fields
                    .iter()
                    .map(|(key, value)| format!("{key}: {}", render_value(heap, value)))
                    .collect::<Vec<_>>();
                entries.sort();
                format!(
                    "{}::{} {{ {} }}",
                    variant.enum_name,
                    variant.variant_name,
                    entries.join(", ")
                )
            }
        }
        Value::Function(function) => match &function.name {
            Some(name) => format!("<fn {name}>"),
            None => "<lambda>".into(),
        },
        Value::BoundMethod(method) => match &method.method.name {
            Some(name) => format!("<bound {name}>"),
            None => "<bound method>".into(),
        },
        Value::CompiledFunction(function) => match &function.name {
            Some(name) => format!("<compiled {name}>"),
            None => "<compiled lambda>".into(),
        },
        Value::NativeFunction(native) => format!("<native {}>", native.name()),
        Value::VmBoundMethod(method) => match &method.method.name {
            Some(name) => format!("<bound {name}>"),
            None => "<bound vm method>".into(),
        },
        Value::Builtin(_) => "<builtin>".into(),
        Value::Null => "null".into(),
    }
}

pub fn string_value(heap: &mut GcHeap, value: impl Into<String>) -> Value {
    Value::String(heap.alloc_string(value.into()))
}

pub fn array_value(heap: &mut GcHeap, items: Vec<Value>) -> Value {
    Value::Array(heap.alloc_array(items))
}

pub fn map_value(heap: &mut GcHeap, items: HashMap<String, Value>) -> Value {
    Value::Map(heap.alloc_map(items))
}

pub fn closure_value(
    heap: &mut GcHeap,
    function: Rc<CompiledFunction>,
    upvalues: Vec<Rc<RefCell<Value>>>,
) -> Value {
    Value::Closure(heap.alloc_closure(ClosureObject { function, upvalues }))
}

pub fn struct_instance_value(
    heap: &mut GcHeap,
    type_name: String,
    fields: HashMap<String, Value>,
) -> Value {
    Value::StructInstance(heap.alloc_struct_instance(StructInstanceObject { type_name, fields }))
}

pub fn enum_variant_value(
    heap: &mut GcHeap,
    enum_name: String,
    variant_name: String,
    fields: HashMap<String, Value>,
) -> Value {
    Value::EnumVariant(heap.alloc_enum_variant(EnumVariantObject {
        enum_name,
        variant_name,
        fields,
    }))
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

//! Environment 與執行期值。
//!
//! 這裡負責變數作用域、閉包捕獲環境，以及執行期 Value 型別。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Statement;
use crate::error::{Result, TinyLangError};

/// 陣列採共享可變參考，讓函式與閉包能共用同一份資料。
pub type ArrayRef = Rc<RefCell<Vec<Value>>>;

/// Map 也採共享可變參考，方便索引寫入與跨作用域共享。
pub type MapRef = Rc<RefCell<HashMap<String, Value>>>;

/// 執行期會流動的值。
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    String(String),
    Bool(bool),
    Array(ArrayRef),
    Map(MapRef),
    Function(FunctionValue),
    Builtin(BuiltinFunction),
    Null,
}

/// 使用者定義函式或 lambda 的執行期表示。
#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
    pub closure: EnvRef,
}

/// 內建函式清單。
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
}

pub type EnvRef = Rc<RefCell<Environment>>;

#[derive(Debug, Clone)]
pub struct Environment {
    values: HashMap<String, Value>,
    parent: Option<EnvRef>,
}

impl Environment {
    pub fn new() -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            parent: None,
        }))
    }

    pub fn enclosed(parent: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            parent: Some(parent),
        }))
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Result<Value> {
        if let Some(value) = self.values.get(name) {
            return Ok(value.clone());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow().get(name);
        }

        Err(TinyLangError::runtime(format!("Variable '{name}' not defined")))
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            return Ok(());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }

        Err(TinyLangError::runtime(format!("Variable '{name}' not defined")))
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
            Value::Function(_) | Value::Builtin(_) => true,
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
            Value::Function(_) | Value::Builtin(_) => "Function",
            Value::Null => "Null",
        }
    }

    pub fn type_name_for_builtin(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::String(_) => "string",
            Value::Bool(_) => "bool",
            Value::Array(_) => "array",
            Value::Map(_) => "map",
            Value::Function(_) | Value::Builtin(_) => "function",
            Value::Null => "null",
        }
    }
}

impl PartialEq for FunctionValue {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.params == other.params && self.body == other.body
    }
}

impl Eq for FunctionValue {}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Array(a), Value::Array(b)) => *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
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
            Value::Function(function) => match &function.name {
                Some(name) => write!(f, "<fn {name}>"),
                None => write!(f, "<lambda>"),
            },
            Value::Builtin(_) => write!(f, "<builtin>"),
            Value::Null => write!(f, "null"),
        }
    }
}

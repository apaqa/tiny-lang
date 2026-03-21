//! Environment（執行期作用域）。
//!
//! 這裡管理變數與函式的查找。
//! Phase 2 的陣列是可變資料，因此會用共享所有權保存。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Statement;
use crate::error::{Result, TinyLangError};

/// 陣列的共享表示。
pub type ArrayRef = Rc<RefCell<Vec<Value>>>;

/// 執行期值。
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    String(String),
    Bool(bool),
    Array(ArrayRef),
    Function(FunctionValue),
    Builtin(BuiltinFunction),
    Null,
}

/// 使用者定義函式。
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionValue {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

/// 內建函式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Len,
    Push,
    Pop,
    Str,
    Int,
    TypeOf,
    Input,
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

        Err(TinyLangError::runtime(format!(
            "Variable '{name}' not defined"
        )))
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            return Ok(());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }

        Err(TinyLangError::runtime(format!(
            "Variable '{name}' not defined"
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
            Value::Function(_) | Value::Builtin(_) => "function",
            Value::Null => "null",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Array(a), Value::Array(b)) => *a.borrow() == *b.borrow(),
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
            Value::Function(function) => write!(f, "<fn {}>", function.name),
            Value::Builtin(_) => write!(f, "<builtin>"),
            Value::Null => write!(f, "null"),
        }
    }
}

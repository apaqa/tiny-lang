//! Environment（執行期作用域）。
//!
//! 使用 HashMap 儲存當前 scope 的值，
//! 並用 parent 鏈往外查找。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Statement;
use crate::error::{Result, TinyLangError};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    String(String),
    Bool(bool),
    Function(FunctionValue),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionValue {
    pub params: Vec<String>,
    pub body: Vec<Statement>,
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

        Err(TinyLangError::Runtime(format!("未定義的識別字: {name}")))
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            return Ok(());
        }

        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }

        Err(TinyLangError::Runtime(format!(
            "不能指派給未宣告的變數: {name}"
        )))
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(value) => *value,
            Value::Int(value) => *value != 0,
            Value::String(value) => !value.is_empty(),
            Value::Function(_) => true,
            Value::Null => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Function(_) => write!(f, "<fn>"),
            Value::Null => write!(f, "null"),
        }
    }
}

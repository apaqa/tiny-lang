//! Tree-walking interpreter。
//!
//! Phase 2 新增：
//! - 陣列與索引
//! - 內建函式
//! - 字串索引 / 字串比較
//! - 更清楚的型別錯誤訊息

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Cursor, Stdin, Stdout, Write};
use std::rc::Rc;

use crate::ast::{BinaryOperator, Expr, Program, Statement, UnaryOperator};
use crate::environment::{BuiltinFunction, EnvRef, Environment, FunctionValue, Value};
use crate::error::{Result, TinyLangError};

#[derive(Debug)]
enum Signal {
    Return(Value),
    Error(TinyLangError),
}

impl From<TinyLangError> for Signal {
    fn from(value: TinyLangError) -> Self {
        Signal::Error(value)
    }
}

type RuntimeResult<T> = std::result::Result<T, Signal>;

pub struct Interpreter<W: Write, R: BufRead> {
    env: EnvRef,
    output: W,
    input: R,
}

impl Interpreter<Stdout, BufReader<Stdin>> {
    pub fn new() -> Self {
        Self::with_io(std::io::stdout(), BufReader::new(std::io::stdin()))
    }
}

impl<W: Write> Interpreter<W, Cursor<Vec<u8>>> {
    pub fn with_output(output: W) -> Self {
        Self::with_io(output, Cursor::new(Vec::new()))
    }
}

impl<W: Write, R: BufRead> Interpreter<W, R> {
    pub fn with_io(output: W, input: R) -> Self {
        let env = Environment::new();
        install_builtins(&env);
        Self { env, output, input }
    }

    pub fn interpret(&mut self, program: &Program) -> Result<()> {
        match self.execute_block(program, self.env.clone()) {
            Ok(_) => Ok(()),
            Err(Signal::Error(err)) => Err(err),
            Err(Signal::Return(_)) => Err(TinyLangError::runtime(
                "return can only appear inside a function",
            )),
        }
    }

    pub fn interpret_source(&mut self, source: &str) -> Result<()> {
        let program = crate::parse_source(source)?;
        self.interpret(&program)
    }

    fn execute_block(&mut self, statements: &[Statement], env: EnvRef) -> RuntimeResult<Value> {
        let previous = self.env.clone();
        self.env = env;

        let result = (|| {
            for statement in statements {
                self.execute_statement(statement)?;
            }
            Ok(Value::Null)
        })();

        self.env = previous;
        result
    }

    fn execute_statement(&mut self, statement: &Statement) -> RuntimeResult<Value> {
        match statement {
            Statement::LetDecl { name, value } => {
                let value = self.evaluate_expr(value)?;
                self.env.borrow_mut().define(name.clone(), value);
                Ok(Value::Null)
            }
            Statement::Assignment { name, value } => {
                let value = self.evaluate_expr(value)?;
                self.env.borrow_mut().assign(name, value)?;
                Ok(Value::Null)
            }
            Statement::IndexAssignment { array, index, value } => {
                let array_value = self.env.borrow().get(array)?;
                let index_value = self.evaluate_expr(index)?;
                let value = self.evaluate_expr(value)?;
                self.assign_index(array_value, index_value, value)
            }
            Statement::FnDecl { name, params, body } => {
                self.env.borrow_mut().define(
                    name.clone(),
                    Value::Function(FunctionValue {
                        name: name.clone(),
                        params: params.clone(),
                        body: body.clone(),
                    }),
                );
                Ok(Value::Null)
            }
            Statement::Return(expr) => {
                let value = self.evaluate_expr(expr)?;
                Err(Signal::Return(value))
            }
            Statement::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                if self.evaluate_expr(condition)?.is_truthy() {
                    self.execute_block(then_body, Environment::enclosed(self.env.clone()))?;
                } else if let Some(else_body) = else_body {
                    self.execute_block(else_body, Environment::enclosed(self.env.clone()))?;
                }
                Ok(Value::Null)
            }
            Statement::While { condition, body } => {
                while self.evaluate_expr(condition)?.is_truthy() {
                    self.execute_block(body, Environment::enclosed(self.env.clone()))?;
                }
                Ok(Value::Null)
            }
            Statement::Print(expr) => {
                let value = self.evaluate_expr(expr)?;
                writeln!(self.output, "{value}")
                    .map_err(|err| Signal::Error(TinyLangError::io(err.to_string())))?;
                Ok(Value::Null)
            }
            Statement::ExprStatement(expr) => {
                self.evaluate_expr(expr)?;
                Ok(Value::Null)
            }
        }
    }

    fn evaluate_expr(&mut self, expr: &Expr) -> RuntimeResult<Value> {
        match expr {
            Expr::IntLit(value) => Ok(Value::Int(*value)),
            Expr::StringLit(value) => Ok(Value::String(value.clone())),
            Expr::BoolLit(value) => Ok(Value::Bool(*value)),
            Expr::Ident(name) => Ok(self.env.borrow().get(name)?),
            Expr::ArrayLit(items) => {
                let mut values = Vec::new();
                for item in items {
                    values.push(self.evaluate_expr(item)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expr::IndexAccess { array, index } => {
                let array_value = self.evaluate_expr(array)?;
                let index_value = self.evaluate_expr(index)?;
                self.read_index(array_value, index_value)
            }
            Expr::UnaryOp { op, operand } => {
                let value = self.evaluate_expr(operand)?;
                self.eval_unary(*op, value)
            }
            Expr::BinaryOp { left, op, right } => {
                let left_value = self.evaluate_expr(left)?;

                match op {
                    BinaryOperator::And => {
                        if !left_value.is_truthy() {
                            return Ok(Value::Bool(false));
                        }
                        let right_value = self.evaluate_expr(right)?;
                        return Ok(Value::Bool(right_value.is_truthy()));
                    }
                    BinaryOperator::Or => {
                        if left_value.is_truthy() {
                            return Ok(Value::Bool(true));
                        }
                        let right_value = self.evaluate_expr(right)?;
                        return Ok(Value::Bool(right_value.is_truthy()));
                    }
                    _ => {}
                }

                let right_value = self.evaluate_expr(right)?;
                self.eval_binary(left_value, *op, right_value)
            }
            Expr::FnCall { name, args } => self.call_function(name, args),
        }
    }

    fn call_function(&mut self, name: &str, args: &[Expr]) -> RuntimeResult<Value> {
        let callable = self.env.borrow().get(name)?;
        match callable {
            Value::Function(function) => self.call_user_function(&function, args),
            Value::Builtin(builtin) => self.call_builtin(name, builtin, args),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "'{name}' is not callable, got {}",
                other.type_name()
            )))),
        }
    }

    fn call_user_function(&mut self, function: &FunctionValue, args: &[Expr]) -> RuntimeResult<Value> {
        if function.params.len() != args.len() {
            return Err(Signal::Error(TinyLangError::runtime(format!(
                "Function '{}' expects {} arguments, got {}",
                function.name,
                function.params.len(),
                args.len()
            ))));
        }

        let call_env = Environment::enclosed(self.env.clone());
        for (param, arg_expr) in function.params.iter().zip(args.iter()) {
            let value = self.evaluate_expr(arg_expr)?;
            call_env.borrow_mut().define(param.clone(), value);
        }

        match self.execute_block(&function.body, call_env) {
            Ok(value) => Ok(value),
            Err(Signal::Return(value)) => Ok(value),
            Err(err) => Err(err),
        }
    }

    fn call_builtin(&mut self, name: &str, builtin: BuiltinFunction, args: &[Expr]) -> RuntimeResult<Value> {
        match builtin {
            BuiltinFunction::Len => {
                let values = self.eval_args(name, 1, args)?;
                match &values[0] {
                    Value::Array(items) => Ok(Value::Int(items.borrow().len() as i64)),
                    Value::String(value) => Ok(Value::Int(value.chars().count() as i64)),
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'len' expects Array or String, got {}",
                        other.type_name()
                    )))),
                }
            }
            BuiltinFunction::Push => {
                let values = self.eval_args(name, 2, args)?;
                match &values[0] {
                    Value::Array(items) => {
                        items.borrow_mut().push(values[1].clone());
                        Ok(Value::Null)
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'push' expects Array as first argument, got {}",
                        other.type_name()
                    )))),
                }
            }
            BuiltinFunction::Pop => {
                let values = self.eval_args(name, 1, args)?;
                match &values[0] {
                    Value::Array(items) => Ok(items.borrow_mut().pop().unwrap_or(Value::Null)),
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'pop' expects Array, got {}",
                        other.type_name()
                    )))),
                }
            }
            BuiltinFunction::Str => {
                let values = self.eval_args(name, 1, args)?;
                Ok(Value::String(values[0].to_string()))
            }
            BuiltinFunction::Int => {
                let values = self.eval_args(name, 1, args)?;
                self.cast_to_int(&values[0])
            }
            BuiltinFunction::TypeOf => {
                let values = self.eval_args(name, 1, args)?;
                Ok(Value::String(values[0].type_name_for_builtin().to_string()))
            }
            BuiltinFunction::Input => {
                let values = self.eval_args(name, 1, args)?;
                let prompt = match &values[0] {
                    Value::String(prompt) => prompt.clone(),
                    other => {
                        return Err(Signal::Error(TinyLangError::runtime(format!(
                            "Function 'input' expects String prompt, got {}",
                            other.type_name()
                        ))))
                    }
                };

                write!(self.output, "{prompt}")
                    .and_then(|_| self.output.flush())
                    .map_err(|err| Signal::Error(TinyLangError::io(err.to_string())))?;

                let mut line = String::new();
                self.input
                    .read_line(&mut line)
                    .map_err(|err| Signal::Error(TinyLangError::io(err.to_string())))?;

                while matches!(line.chars().last(), Some('\n' | '\r')) {
                    line.pop();
                }

                Ok(Value::String(line))
            }
        }
    }

    fn eval_args(&mut self, name: &str, expected: usize, args: &[Expr]) -> RuntimeResult<Vec<Value>> {
        if expected != args.len() {
            return Err(Signal::Error(TinyLangError::runtime(format!(
                "Function '{name}' expects {expected} arguments, got {}",
                args.len()
            ))));
        }

        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.evaluate_expr(arg)?);
        }
        Ok(values)
    }

    fn cast_to_int(&self, value: &Value) -> RuntimeResult<Value> {
        match value {
            Value::Int(value) => Ok(Value::Int(*value)),
            Value::Bool(value) => Ok(Value::Int(if *value { 1 } else { 0 })),
            Value::String(value) => value
                .parse::<i64>()
                .map(Value::Int)
                .map_err(|_| Signal::Error(TinyLangError::runtime(format!(
                    "Cannot convert String '{}' to Int",
                    value
                )))),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Cannot convert {} to Int",
                other.type_name()
            )))),
        }
    }

    fn assign_index(&mut self, target: Value, index: Value, value: Value) -> RuntimeResult<Value> {
        let idx = self.expect_index(index)?;
        match target {
            Value::Array(items) => {
                let mut items = items.borrow_mut();
                if idx >= items.len() {
                    return Err(Signal::Error(TinyLangError::runtime(format!(
                        "Index out of bounds: array length is {}, index is {}",
                        items.len(),
                        idx
                    ))));
                }
                items[idx] = value;
                Ok(Value::Null)
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index assignment expects Array, got {}",
                other.type_name()
            )))),
        }
    }

    fn read_index(&self, target: Value, index: Value) -> RuntimeResult<Value> {
        let idx = self.expect_index(index)?;
        match target {
            Value::Array(items) => {
                let items = items.borrow();
                items.get(idx).cloned().ok_or_else(|| {
                    Signal::Error(TinyLangError::runtime(format!(
                        "Index out of bounds: array length is {}, index is {}",
                        items.len(),
                        idx
                    )))
                })
            }
            Value::String(text) => {
                let chars: Vec<char> = text.chars().collect();
                chars.get(idx).map(|ch| Value::String(ch.to_string())).ok_or_else(|| {
                    Signal::Error(TinyLangError::runtime(format!(
                        "Index out of bounds: string length is {}, index is {}",
                        chars.len(),
                        idx
                    )))
                })
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index access expects Array or String, got {}",
                other.type_name()
            )))),
        }
    }

    fn expect_index(&self, value: Value) -> RuntimeResult<usize> {
        match value {
            Value::Int(index) if index >= 0 => Ok(index as usize),
            Value::Int(index) => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index must be non-negative, got {}",
                index
            )))),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index must be Int, got {}",
                other.type_name()
            )))),
        }
    }

    fn eval_unary(&self, op: UnaryOperator, value: Value) -> RuntimeResult<Value> {
        match op {
            UnaryOperator::Neg => match value {
                Value::Int(v) => Ok(Value::Int(-v)),
                other => Err(Signal::Error(TinyLangError::runtime(format!(
                    "Cannot negate {}",
                    other.type_name()
                )))),
            },
            UnaryOperator::Not => Ok(Value::Bool(!value.is_truthy())),
        }
    }

    fn eval_binary(&self, left: Value, op: BinaryOperator, right: Value) -> RuntimeResult<Value> {
        match op {
            BinaryOperator::Add => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                (a, b) => Err(Signal::Error(TinyLangError::runtime(format!(
                    "Cannot add {} and {}",
                    a.type_name(),
                    b.type_name()
                )))),
            },
            BinaryOperator::Sub => self.eval_int_binary(left, right, |a, b| a - b, "subtract"),
            BinaryOperator::Mul => self.eval_int_binary(left, right, |a, b| a * b, "multiply"),
            BinaryOperator::Div => {
                let (a, b) = self.as_int_pair(left, right, "divide")?;
                if b == 0 {
                    return Err(Signal::Error(TinyLangError::runtime("Cannot divide by zero")));
                }
                Ok(Value::Int(a / b))
            }
            BinaryOperator::Mod => {
                let (a, b) = self.as_int_pair(left, right, "modulo")?;
                if b == 0 {
                    return Err(Signal::Error(TinyLangError::runtime("Cannot modulo by zero")));
                }
                Ok(Value::Int(a % b))
            }
            BinaryOperator::Eq => Ok(Value::Bool(left == right)),
            BinaryOperator::Ne => Ok(Value::Bool(left != right)),
            BinaryOperator::Lt => self.eval_int_compare(left, right, |a, b| a < b, "<"),
            BinaryOperator::Gt => self.eval_int_compare(left, right, |a, b| a > b, ">"),
            BinaryOperator::Le => self.eval_int_compare(left, right, |a, b| a <= b, "<="),
            BinaryOperator::Ge => self.eval_int_compare(left, right, |a, b| a >= b, ">="),
            BinaryOperator::And | BinaryOperator::Or => unreachable!(),
        }
    }

    fn as_int_pair(&self, left: Value, right: Value, action: &str) -> RuntimeResult<(i64, i64)> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok((a, b)),
            (a, b) => Err(Signal::Error(TinyLangError::runtime(format!(
                "Cannot {action} {} and {}",
                a.type_name(),
                b.type_name()
            )))),
        }
    }

    fn eval_int_binary<F>(&self, left: Value, right: Value, f: F, action: &str) -> RuntimeResult<Value>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let (a, b) = self.as_int_pair(left, right, action)?;
        Ok(Value::Int(f(a, b)))
    }

    fn eval_int_compare<F>(&self, left: Value, right: Value, f: F, op_name: &str) -> RuntimeResult<Value>
    where
        F: FnOnce(i64, i64) -> bool,
    {
        let (a, b) = self.as_int_pair(left, right, &format!("compare with '{op_name}'"))?;
        Ok(Value::Bool(f(a, b)))
    }
}

fn install_builtins(env: &EnvRef) {
    let mut env = env.borrow_mut();
    env.define("len".into(), Value::Builtin(BuiltinFunction::Len));
    env.define("push".into(), Value::Builtin(BuiltinFunction::Push));
    env.define("pop".into(), Value::Builtin(BuiltinFunction::Pop));
    env.define("str".into(), Value::Builtin(BuiltinFunction::Str));
    env.define("int".into(), Value::Builtin(BuiltinFunction::Int));
    env.define("type_of".into(), Value::Builtin(BuiltinFunction::TypeOf));
    env.define("input".into(), Value::Builtin(BuiltinFunction::Input));
}

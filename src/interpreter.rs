//! Tree-walking interpreter。
//!
//! 這種做法不會先編譯成 bytecode，
//! 而是直接遞迴走訪 AST 並執行每個節點。

use std::io::Write;

use crate::ast::{BinaryOperator, Expr, Program, Statement, UnaryOperator};
use crate::environment::{EnvRef, Environment, FunctionValue, Value};
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

pub struct Interpreter<W: Write> {
    env: EnvRef,
    output: W,
}

impl Interpreter<std::io::Stdout> {
    pub fn new() -> Self {
        Self::with_output(std::io::stdout())
    }
}

impl<W: Write> Interpreter<W> {
    pub fn with_output(output: W) -> Self {
        Self {
            env: Environment::new(),
            output,
        }
    }

    pub fn interpret(&mut self, program: &Program) -> Result<()> {
        match self.execute_block(program, self.env.clone()) {
            Ok(_) => Ok(()),
            Err(Signal::Error(err)) => Err(err),
            Err(Signal::Return(_)) => Err(TinyLangError::Runtime(
                "return 只能出現在函式內部".into(),
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
            Statement::FnDecl { name, params, body } => {
                self.env.borrow_mut().define(
                    name.clone(),
                    Value::Function(FunctionValue {
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
                    let env = Environment::enclosed(self.env.clone());
                    self.execute_block(then_body, env)?;
                } else if let Some(else_body) = else_body {
                    let env = Environment::enclosed(self.env.clone());
                    self.execute_block(else_body, env)?;
                }
                Ok(Value::Null)
            }
            Statement::While { condition, body } => {
                while self.evaluate_expr(condition)?.is_truthy() {
                    let env = Environment::enclosed(self.env.clone());
                    self.execute_block(body, env)?;
                }
                Ok(Value::Null)
            }
            Statement::Print(expr) => {
                let value = self.evaluate_expr(expr)?;
                writeln!(self.output, "{value}")
                    .map_err(|err| Signal::Error(TinyLangError::Io(err.to_string())))?;
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
        let function = self.env.borrow().get(name)?;
        let Value::Function(function) = function else {
            return Err(Signal::Error(TinyLangError::Runtime(format!(
                "{name} 不是函式"
            ))));
        };

        if function.params.len() != args.len() {
            return Err(Signal::Error(TinyLangError::Runtime(format!(
                "函式 {name} 需要 {} 個參數，但收到 {} 個",
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

    fn eval_unary(&self, op: UnaryOperator, value: Value) -> RuntimeResult<Value> {
        match op {
            UnaryOperator::Neg => match value {
                Value::Int(v) => Ok(Value::Int(-v)),
                other => Err(Signal::Error(TinyLangError::Runtime(format!(
                    "負號只支援整數，實際為 {other:?}"
                )))),
            },
            UnaryOperator::Not => Ok(Value::Bool(!value.is_truthy())),
        }
    }

    fn eval_binary(
        &self,
        left: Value,
        op: BinaryOperator,
        right: Value,
    ) -> RuntimeResult<Value> {
        match op {
            BinaryOperator::Add => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                (Value::String(a), Value::Int(b)) => Ok(Value::String(format!("{a}{b}"))),
                (Value::Int(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
                (a, b) => Err(Signal::Error(TinyLangError::Runtime(format!(
                    "不支援的加法運算: {a:?} + {b:?}"
                )))),
            },
            BinaryOperator::Sub => self.eval_int_binary(left, right, |a, b| a - b, "-"),
            BinaryOperator::Mul => self.eval_int_binary(left, right, |a, b| a * b, "*"),
            BinaryOperator::Div => {
                let (a, b) = self.as_int_pair(left, right, "/")?;
                if b == 0 {
                    return Err(Signal::Error(TinyLangError::Runtime("不能除以 0".into())));
                }
                Ok(Value::Int(a / b))
            }
            BinaryOperator::Mod => {
                let (a, b) = self.as_int_pair(left, right, "%")?;
                if b == 0 {
                    return Err(Signal::Error(TinyLangError::Runtime("不能對 0 取餘數".into())));
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

    fn as_int_pair(&self, left: Value, right: Value, op_name: &str) -> RuntimeResult<(i64, i64)> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok((a, b)),
            (a, b) => Err(Signal::Error(TinyLangError::Runtime(format!(
                "運算子 {op_name} 只支援整數，實際為 {a:?} 和 {b:?}"
            )))),
        }
    }

    fn eval_int_binary<F>(
        &self,
        left: Value,
        right: Value,
        f: F,
        op_name: &str,
    ) -> RuntimeResult<Value>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let (a, b) = self.as_int_pair(left, right, op_name)?;
        Ok(Value::Int(f(a, b)))
    }

    fn eval_int_compare<F>(
        &self,
        left: Value,
        right: Value,
        f: F,
        op_name: &str,
    ) -> RuntimeResult<Value>
    where
        F: FnOnce(i64, i64) -> bool,
    {
        let (a, b) = self.as_int_pair(left, right, op_name)?;
        Ok(Value::Bool(f(a, b)))
    }
}

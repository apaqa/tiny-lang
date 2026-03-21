//! Tree-walking interpreter。
//!
//! Phase 4 在這裡加入型別檢查、import 與擴充標準庫。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Stdin, Stdout, Write};
use std::path::{Path, PathBuf};

use crate::ast::{BinaryOperator, Expr, Pattern, Program, Statement, TypeAnnotation, UnaryOperator};
use crate::environment::{
    array_value, enum_variant_value, map_value, render_value, string_value, struct_instance_value,
    BuiltinFunction, EnumDef, EnvRef, Environment, FunctionValue, StructDef, Value,
};
use crate::error::{Result, TinyLangError};
use crate::gc::{GcHeap, GcStructRef};

/// 迴圈與函式共用的控制流程訊號。
#[derive(Debug, Clone)]
enum ControlFlow {
    Break,
    Continue,
    Return(Value),
}

#[derive(Debug)]
enum Signal {
    Control(ControlFlow),
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
    current_dir: PathBuf,
    imported_files: HashSet<PathBuf>,
    heap: GcHeap,
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
        Self {
            env,
            output,
            input,
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            imported_files: HashSet::new(),
            heap: GcHeap::new(),
        }
    }

    pub fn interpret(&mut self, program: &Program) -> Result<()> {
        match self.execute_block(program, self.env.clone()) {
            Ok(_) => Ok(()),
            Err(Signal::Error(err)) => Err(err),
            Err(Signal::Control(ControlFlow::Return(_))) => {
                Err(TinyLangError::runtime("return can only appear inside a function"))
            }
            Err(Signal::Control(ControlFlow::Break)) => {
                Err(TinyLangError::runtime("break can only appear inside a loop"))
            }
            Err(Signal::Control(ControlFlow::Continue)) => {
                Err(TinyLangError::runtime("continue can only appear inside a loop"))
            }
        }
    }

    pub fn interpret_source(&mut self, source: &str) -> Result<()> {
        let program = crate::parse_source(source)?;
        self.interpret(&program)
    }

    pub fn interpret_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|err| TinyLangError::io(err.to_string()))?;
        let previous_dir = self.current_dir.clone();
        if let Some(parent) = path.parent() {
            self.current_dir = parent.to_path_buf();
        }
        let result = self.interpret_source(&source);
        self.current_dir = previous_dir;
        result
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
            Statement::Import { path } => self.execute_import(path),
            Statement::StructDecl { name, fields } => {
                self.env.borrow_mut().define_struct(
                    name.clone(),
                    StructDef {
                        name: name.clone(),
                        fields: fields.clone(),
                    },
                );
                Ok(Value::Null)
            }
            Statement::EnumDecl { name, variants } => {
                use crate::environment::EnumVariantDef;
                let enum_def = EnumDef {
                    name: name.clone(),
                    variants: variants
                        .iter()
                        .map(|v| EnumVariantDef {
                            name: v.name.clone(),
                            fields: v.fields.clone(),
                        })
                        .collect(),
                };
                self.env.borrow_mut().define_enum(name.clone(), enum_def);
                Ok(Value::Null)
            }
            Statement::LetDecl {
                name,
                type_annotation,
                value,
            } => {
                let value = self.evaluate_expr(value)?;
                if let Some(annotation) = type_annotation {
                    self.ensure_type_matches(annotation, &value)?;
                }
                self.env
                    .borrow_mut()
                    .define_typed(name.clone(), value, type_annotation.clone());
                Ok(Value::Null)
            }
            Statement::Assignment { name, value } => {
                let value = self.evaluate_expr(value)?;
                if let Some(annotation) = self.env.borrow().get_annotation(name)? {
                    self.ensure_type_matches(&annotation, &value)?;
                }
                self.env.borrow_mut().assign(name, value)?;
                Ok(Value::Null)
            }
            Statement::IndexAssignment { target, index, value } => {
                let target_value = self.evaluate_expr(target)?;
                let index_value = self.evaluate_expr(index)?;
                let value = self.evaluate_expr(value)?;
                self.assign_index(target_value, index_value, value)
            }
            Statement::FieldAssignment { object, field, value } => {
                let object_value = self.evaluate_expr(object)?;
                let value = self.evaluate_expr(value)?;
                self.assign_field(object_value, field, value)
            }
            Statement::FnDecl {
                name,
                params,
                return_type,
                body,
            } => {
                let function = Value::Function(FunctionValue {
                    name: Some(name.clone()),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: body.clone(),
                    closure: self.env.clone(),
                });
                self.env.borrow_mut().define(name.clone(), function);
                Ok(Value::Null)
            }
            Statement::MethodDecl {
                struct_name,
                method_name,
                params,
                body,
                return_type,
            } => {
                let function = FunctionValue {
                    name: Some(format!("{struct_name}.{method_name}")),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: body.clone(),
                    closure: self.env.clone(),
                };
                self.env
                    .borrow_mut()
                    .define_method(struct_name.clone(), method_name.clone(), function);
                Ok(Value::Null)
            }
            Statement::Return(expr) => {
                let value = self.evaluate_expr(expr)?;
                Err(Signal::Control(ControlFlow::Return(value)))
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
                    match self.execute_block(body, Environment::enclosed(self.env.clone())) {
                        Ok(_) => {}
                        Err(Signal::Control(ControlFlow::Break)) => break,
                        Err(Signal::Control(ControlFlow::Continue)) => continue,
                        Err(other) => return Err(other),
                    }
                }
                Ok(Value::Null)
            }
            Statement::ForLoop {
                variable,
                iterable,
                body,
            } => {
                let iterable_value = self.evaluate_expr(iterable)?;
                let items = self.iterate_values(iterable_value)?;
                for item in items {
                    let loop_env = Environment::enclosed(self.env.clone());
                    loop_env.borrow_mut().define(variable.clone(), item);
                    match self.execute_block(body, loop_env) {
                        Ok(_) => {}
                        Err(Signal::Control(ControlFlow::Break)) => break,
                        Err(Signal::Control(ControlFlow::Continue)) => continue,
                        Err(other) => return Err(other),
                    }
                }
                Ok(Value::Null)
            }
            Statement::Break => Err(Signal::Control(ControlFlow::Break)),
            Statement::Continue => Err(Signal::Control(ControlFlow::Continue)),
            Statement::TryCatch {
                try_body,
                catch_var,
                catch_body,
            } => match self.execute_block(try_body, Environment::enclosed(self.env.clone())) {
                Ok(_) => Ok(Value::Null),
                Err(Signal::Error(err)) => {
                    let catch_env = Environment::enclosed(self.env.clone());
                    let err_str = err.to_string();
                    let err_val = string_value(&mut self.heap, err_str);
                    catch_env
                        .borrow_mut()
                        .define(catch_var.clone(), err_val);
                    self.execute_block(catch_body, catch_env)
                }
                Err(other) => Err(other),
            },
            Statement::Match { expr, arms } => self.execute_match_statement(expr, arms),
            Statement::Print(expr) => {
                let value = self.evaluate_expr(expr)?;
                let rendered = render_value(&self.heap, &value);
                writeln!(self.output, "{rendered}")
                    .map_err(|err| Signal::Error(TinyLangError::io(err.to_string())))?;
                Ok(Value::Null)
            }
            Statement::ExprStatement(expr) => {
                self.evaluate_expr(expr)?;
                Ok(Value::Null)
            }
        }
    }

    fn execute_import(&mut self, path: &str) -> RuntimeResult<Value> {
        let candidate = self.current_dir.join(path);
        let canonical = fs::canonicalize(&candidate).map_err(|_| {
            Signal::Error(TinyLangError::runtime(format!("Import file not found: {path}")))
        })?;

        if self.imported_files.contains(&canonical) {
            return Ok(Value::Null);
        }

        let source = fs::read_to_string(&canonical)
            .map_err(|err| Signal::Error(TinyLangError::io(err.to_string())))?;
        let program = crate::parse_source(&source).map_err(Signal::Error)?;

        self.imported_files.insert(canonical.clone());
        let previous_dir = self.current_dir.clone();
        if let Some(parent) = canonical.parent() {
            self.current_dir = parent.to_path_buf();
        }

        let result = self.execute_block(&program, self.env.clone());
        self.current_dir = previous_dir;

        if result.is_err() {
            self.imported_files.remove(&canonical);
        }

        result
    }

    fn evaluate_expr(&mut self, expr: &Expr) -> RuntimeResult<Value> {
        match expr {
            Expr::IntLit(value) => Ok(Value::Int(*value)),
            Expr::StringLit(value) => Ok(string_value(&mut self.heap, value.clone())),
            Expr::BoolLit(value) => Ok(Value::Bool(*value)),
            Expr::Ident(name) => Ok(self.env.borrow().get(name)?),
            Expr::StructInit { name, fields } => self.create_struct_instance(name, fields),
            Expr::EnumVariant { enum_name, variant, fields } => {
                self.create_enum_variant(enum_name, variant, fields.as_deref())
            }
            Expr::ArrayLit(items) => {
                let mut values = Vec::new();
                for item in items {
                    values.push(self.evaluate_expr(item)?);
                }
                Ok(array_value(&mut self.heap, values))
            }
            Expr::MapLit(items) => {
                let mut values = HashMap::new();
                for (key_expr, value_expr) in items {
                    let key_value = self.evaluate_expr(key_expr)?;
                    let key = self.expect_map_key(key_value)?;
                    let value = self.evaluate_expr(value_expr)?;
                    values.insert(key, value);
                }
                Ok(map_value(&mut self.heap, values))
            }
            Expr::IndexAccess { target, index } => {
                let target_value = self.evaluate_expr(target)?;
                let index_value = self.evaluate_expr(index)?;
                self.read_index(target_value, index_value)
            }
            Expr::FieldAccess { object, field } => {
                let object_value = self.evaluate_expr(object)?;
                self.read_field_or_method(object_value, field)
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
            Expr::FnCall { callee, args } => {
                let callable = self.evaluate_expr(callee)?;
                self.call_value(callable, args)
            }
            Expr::Lambda { params, body } => Ok(Value::Function(FunctionValue {
                name: None,
                params: params.iter().map(|name| (name.clone(), None)).collect(),
                return_type: None,
                body: body.clone(),
                closure: self.env.clone(),
            })),
        }
    }

    fn call_value(&mut self, callable: Value, args: &[Expr]) -> RuntimeResult<Value> {
        match callable {
            Value::Function(function) => self.call_user_function(&function, args),
            Value::BoundMethod(method) => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.evaluate_expr(arg)?);
                }
                self.call_method_with_values(&method.method, method.receiver, values)
            }
            Value::Builtin(builtin) => self.call_builtin(builtin, args),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "value of type {} is not callable",
                other.type_name_for_error()
            )))),
        }
    }

    fn call_user_function(&mut self, function: &FunctionValue, args: &[Expr]) -> RuntimeResult<Value> {
        if function.params.len() != args.len() {
            let name = function.name.as_deref().unwrap_or("<lambda>");
            return Err(Signal::Error(TinyLangError::runtime(format!(
                "Function '{name}' expects {} arguments, got {}",
                function.params.len(),
                args.len()
            ))));
        }

        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.evaluate_expr(arg)?);
        }
        self.call_function_with_values(function, values)
    }

    fn call_function_with_values(
        &mut self,
        function: &FunctionValue,
        values: Vec<Value>,
    ) -> RuntimeResult<Value> {
        let call_env = Environment::enclosed(function.closure.clone());
        for ((param, annotation), value) in function.params.iter().zip(values.into_iter()) {
            if let Some(annotation) = annotation {
                self.ensure_type_matches(annotation, &value)?;
            }
            call_env
                .borrow_mut()
                .define_typed(param.clone(), value, annotation.clone());
        }

        let result = match self.execute_block(&function.body, call_env) {
            Ok(value) => value,
            Err(Signal::Control(ControlFlow::Return(value))) => value,
            Err(err) => return Err(err),
        };

        if let Some(annotation) = &function.return_type {
            self.ensure_type_matches(annotation, &result)?;
        }

        Ok(result)
    }

    fn call_method_with_values(
        &mut self,
        function: &FunctionValue,
        receiver: GcStructRef,
        values: Vec<Value>,
    ) -> RuntimeResult<Value> {
        if function.params.len() != values.len() {
            let name = function.name.as_deref().unwrap_or("<method>");
            return Err(Signal::Error(TinyLangError::runtime(format!(
                "Function '{name}' expects {} arguments, got {}",
                function.params.len(),
                values.len()
            ))));
        }

        let call_env = Environment::enclosed(function.closure.clone());
        call_env
            .borrow_mut()
            .define("self".into(), Value::StructInstance(receiver));

        for ((param, annotation), value) in function.params.iter().zip(values.into_iter()) {
            if let Some(annotation) = annotation {
                self.ensure_type_matches(annotation, &value)?;
            }
            call_env
                .borrow_mut()
                .define_typed(param.clone(), value, annotation.clone());
        }

        let result = match self.execute_block(&function.body, call_env) {
            Ok(value) => value,
            Err(Signal::Control(ControlFlow::Return(value))) => value,
            Err(err) => return Err(err),
        };

        if let Some(annotation) = &function.return_type {
            self.ensure_type_matches(annotation, &result)?;
        }

        Ok(result)
    }

    fn call_builtin(&mut self, builtin: BuiltinFunction, args: &[Expr]) -> RuntimeResult<Value> {
        match builtin {
            BuiltinFunction::Len => {
                let values = self.eval_args("len", 1, args)?;
                match &values[0] {
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
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'len' expects Array, String, or Map, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Push => {
                let values = self.eval_args("push", 2, args)?;
                match &values[0] {
                    Value::Array(items) => {
                        let arr_ref = items.clone();
                        let item = values[1].clone();
                        self.heap.with_array_mut(&arr_ref, |arr| arr.push(item));
                        Ok(Value::Null)
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'push' expects Array as first argument, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Pop => {
                let values = self.eval_args("pop", 1, args)?;
                match &values[0] {
                    Value::Array(items) => {
                        let arr_ref = items.clone();
                        Ok(self.heap.with_array_mut(&arr_ref, |arr| arr.pop().unwrap_or(Value::Null)))
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'pop' expects Array, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Str => {
                let values = self.eval_args("str", 1, args)?;
                let rendered = render_value(&self.heap, &values[0]);
                Ok(string_value(&mut self.heap, rendered))
            }
            BuiltinFunction::Int => {
                let values = self.eval_args("int", 1, args)?;
                self.cast_to_int(&values[0])
            }
            BuiltinFunction::TypeOf => {
                let values = self.eval_args("typeof", 1, args)?;
                let type_name = values[0].type_name_for_builtin();
                Ok(string_value(&mut self.heap, type_name))
            }
            BuiltinFunction::Input => {
                let values = self.eval_args("input", 1, args)?;
                let prompt = match &values[0] {
                    Value::String(text) => {
                        let text = text.clone();
                        self.heap.get_string(&text)
                    }
                    other => {
                        return Err(Signal::Error(TinyLangError::runtime(format!(
                            "Function 'input' expects String prompt, got {}",
                            other.type_name_for_error()
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

                Ok(string_value(&mut self.heap, line))
            }
            BuiltinFunction::Range => {
                let values = self.eval_args("range", 2, args)?;
                let start = self.expect_int(values[0].clone(), "range start")?;
                let end = self.expect_int(values[1].clone(), "range end")?;
                let items = (start..end).map(Value::Int).collect::<Vec<_>>();
                Ok(array_value(&mut self.heap, items))
            }
            BuiltinFunction::Keys => {
                let values = self.eval_args("keys", 1, args)?;
                match &values[0] {
                    Value::Map(items) => {
                        let items = items.clone();
                        let mut keys = self.heap.with_map(&items, |m| m.keys().cloned().collect::<Vec<_>>());
                        keys.sort();
                        let mut key_values = Vec::new();
                        for k in keys {
                            key_values.push(string_value(&mut self.heap, k));
                        }
                        Ok(array_value(&mut self.heap, key_values))
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'keys' expects Map, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Values => {
                let values = self.eval_args("values", 1, args)?;
                match &values[0] {
                    Value::Map(items) => {
                        let items = items.clone();
                        let mut entries = self.heap.with_map(&items, |m| {
                            m.iter()
                                .map(|(key, value)| (key.clone(), value.clone()))
                                .collect::<Vec<_>>()
                        });
                        entries.sort_by(|a, b| a.0.cmp(&b.0));
                        let result = entries.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
                        Ok(array_value(&mut self.heap, result))
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'values' expects Map, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Abs => {
                let values = self.eval_args("abs", 1, args)?;
                Ok(Value::Int(self.expect_int(values[0].clone(), "abs argument")?.abs()))
            }
            BuiltinFunction::Max => {
                let values = self.eval_args("max", 2, args)?;
                let a = self.expect_int(values[0].clone(), "max first argument")?;
                let b = self.expect_int(values[1].clone(), "max second argument")?;
                Ok(Value::Int(a.max(b)))
            }
            BuiltinFunction::Min => {
                let values = self.eval_args("min", 2, args)?;
                let a = self.expect_int(values[0].clone(), "min first argument")?;
                let b = self.expect_int(values[1].clone(), "min second argument")?;
                Ok(Value::Int(a.min(b)))
            }
            BuiltinFunction::Pow => {
                let values = self.eval_args("pow", 2, args)?;
                let base = self.expect_int(values[0].clone(), "pow base")?;
                let exp = self.expect_int(values[1].clone(), "pow exponent")?;
                if exp < 0 {
                    return Err(Signal::Error(TinyLangError::runtime(
                        "pow exponent must be non-negative",
                    )));
                }
                Ok(Value::Int(base.pow(exp as u32)))
            }
            BuiltinFunction::Split => {
                let values = self.eval_args("split", 2, args)?;
                let text = self.expect_string(values[0].clone(), "split first argument")?;
                let sep = self.expect_string(values[1].clone(), "split second argument")?;
                let parts_strs: Vec<String> = if sep.is_empty() {
                    text.chars().map(|ch| ch.to_string()).collect()
                } else {
                    text.split(&sep).map(|part| part.to_string()).collect()
                };
                let mut parts = Vec::new();
                for s in parts_strs {
                    parts.push(string_value(&mut self.heap, s));
                }
                Ok(array_value(&mut self.heap, parts))
            }
            BuiltinFunction::Join => {
                let values = self.eval_args("join", 2, args)?;
                let items = self.expect_array(values[0].clone(), "join first argument")?;
                let sep = self.expect_string(values[1].clone(), "join second argument")?;
                let mut rendered_parts = Vec::new();
                for item in &items {
                    rendered_parts.push(render_value(&self.heap, item));
                }
                let result = rendered_parts.join(&sep);
                Ok(string_value(&mut self.heap, result))
            }
            BuiltinFunction::Trim => {
                let values = self.eval_args("trim", 1, args)?;
                let text = self.expect_string(values[0].clone(), "trim argument")?;
                Ok(string_value(&mut self.heap, text.trim().to_string()))
            }
            BuiltinFunction::Upper => {
                let values = self.eval_args("upper", 1, args)?;
                let text = self.expect_string(values[0].clone(), "upper argument")?;
                Ok(string_value(&mut self.heap, text.to_uppercase()))
            }
            BuiltinFunction::Lower => {
                let values = self.eval_args("lower", 1, args)?;
                let text = self.expect_string(values[0].clone(), "lower argument")?;
                Ok(string_value(&mut self.heap, text.to_lowercase()))
            }
            BuiltinFunction::Contains => {
                let values = self.eval_args("contains", 2, args)?;
                let text = self.expect_string(values[0].clone(), "contains first argument")?;
                let needle = self.expect_string(values[1].clone(), "contains second argument")?;
                Ok(Value::Bool(text.contains(&needle)))
            }
            BuiltinFunction::Replace => {
                let values = self.eval_args("replace", 3, args)?;
                let text = self.expect_string(values[0].clone(), "replace first argument")?;
                let old = self.expect_string(values[1].clone(), "replace second argument")?;
                let new = self.expect_string(values[2].clone(), "replace third argument")?;
                Ok(string_value(&mut self.heap, text.replace(&old, &new)))
            }
            BuiltinFunction::Sort => {
                let values = self.eval_args("sort", 1, args)?;
                match &values[0] {
                    Value::Array(items) => {
                        let arr_ref = items.clone();
                        // Check what kind of items we have
                        let all_int = self.heap.with_array(&arr_ref, |arr| {
                            arr.iter().all(|item| matches!(item, Value::Int(_)))
                        });
                        let all_str = self.heap.with_array(&arr_ref, |arr| {
                            arr.iter().all(|item| matches!(item, Value::String(_)))
                        });

                        if all_int {
                            self.heap.with_array_mut(&arr_ref, |arr| {
                                arr.sort_by_key(|item| match item {
                                    Value::Int(value) => *value,
                                    _ => 0,
                                });
                            });
                        } else if all_str {
                            // Extract string keys without borrowing heap mutably through closures
                            let str_refs: Vec<crate::gc::GcStringRef> =
                                self.heap.with_array(&arr_ref, |arr| {
                                    arr.iter()
                                        .map(|item| match item {
                                            Value::String(r) => r.clone(),
                                            _ => unreachable!(),
                                        })
                                        .collect()
                                });
                            let mut string_keys: Vec<(usize, String)> = str_refs
                                .iter()
                                .enumerate()
                                .map(|(i, r)| (i, self.heap.get_string(r)))
                                .collect();
                            string_keys.sort_by(|a, b| a.1.cmp(&b.1));
                            let sorted_indices: Vec<usize> =
                                string_keys.into_iter().map(|(i, _)| i).collect();
                            self.heap.with_array_mut(&arr_ref, |arr| {
                                let original = arr.clone();
                                for (new_pos, old_pos) in sorted_indices.iter().enumerate() {
                                    arr[new_pos] = original[*old_pos].clone();
                                }
                            });
                        } else {
                            return Err(Signal::Error(TinyLangError::runtime(
                                "sort only supports arrays of int or str",
                            )));
                        }
                        Ok(values[0].clone())
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'sort' expects Array, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Reverse => {
                let values = self.eval_args("reverse", 1, args)?;
                match &values[0] {
                    Value::Array(items) => {
                        let arr_ref = items.clone();
                        self.heap.with_array_mut(&arr_ref, |arr| arr.reverse());
                        Ok(values[0].clone())
                    }
                    other => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Function 'reverse' expects Array, got {}",
                        other.type_name_for_error()
                    )))),
                }
            }
            BuiltinFunction::Map => {
                let values = self.eval_args("map", 2, args)?;
                let items = self.expect_array(values[0].clone(), "map first argument")?;
                let callback = self.expect_function_value(values[1].clone(), "map second argument")?;
                let mut result = Vec::new();
                for item in items {
                    result.push(self.call_function_with_values(&callback, vec![item])?);
                }
                Ok(array_value(&mut self.heap, result))
            }
            BuiltinFunction::Filter => {
                let values = self.eval_args("filter", 2, args)?;
                let items = self.expect_array(values[0].clone(), "filter first argument")?;
                let callback = self.expect_function_value(values[1].clone(), "filter second argument")?;
                let mut result = Vec::new();
                for item in items {
                    let keep = self.call_function_with_values(&callback, vec![item.clone()])?;
                    if keep.is_truthy() {
                        result.push(item);
                    }
                }
                Ok(array_value(&mut self.heap, result))
            }
            BuiltinFunction::Reduce => {
                let values = self.eval_args("reduce", 3, args)?;
                let items = self.expect_array(values[0].clone(), "reduce first argument")?;
                let callback = self.expect_function_value(values[1].clone(), "reduce second argument")?;
                let mut acc = values[2].clone();
                for item in items {
                    acc = self.call_function_with_values(&callback, vec![acc, item])?;
                }
                Ok(acc)
            }
            BuiltinFunction::Find => {
                let values = self.eval_args("find", 2, args)?;
                let items = self.expect_array(values[0].clone(), "find first argument")?;
                let callback = self.expect_function_value(values[1].clone(), "find second argument")?;
                for item in items {
                    let matched = self.call_function_with_values(&callback, vec![item.clone()])?;
                    if matched.is_truthy() {
                        return Ok(item);
                    }
                }
                Ok(Value::Null)
            }
            BuiltinFunction::Assert => {
                let values = self.eval_args("assert", 2, args)?;
                let message = self.expect_string(values[1].clone(), "assert message")?;
                if values[0].is_truthy() {
                    Ok(Value::Null)
                } else {
                    Err(Signal::Error(TinyLangError::runtime(message)))
                }
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

    fn iterate_values(&mut self, value: Value) -> RuntimeResult<Vec<Value>> {
        match value {
            Value::Array(items) => Ok(self.heap.with_array(&items, |arr| arr.clone())),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "for loop expects Array iterable, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn cast_to_int(&self, value: &Value) -> RuntimeResult<Value> {
        match value {
            Value::Int(value) => Ok(Value::Int(*value)),
            Value::Bool(value) => Ok(Value::Int(if *value { 1 } else { 0 })),
            Value::String(text) => {
                let s = self.heap.get_string(text);
                s.parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| Signal::Error(TinyLangError::runtime(format!(
                        "Cannot convert String '{}' to Int",
                        s
                    ))))
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Cannot convert {} to Int",
                other.type_name_for_error()
            )))),
        }
    }

    fn execute_match_statement(&mut self, expr: &Expr, arms: &[crate::ast::MatchArm]) -> RuntimeResult<Value> {
        let matched_value = self.evaluate_expr(expr)?;

        for arm in arms {
            if let Some(binding) = self.match_pattern(&arm.pattern, &matched_value) {
                let match_env = Environment::enclosed(self.env.clone());
                if let Some((name, value)) = binding {
                    match_env.borrow_mut().define(name, value);
                }
                self.execute_block(&arm.body, match_env)?;
                return Ok(Value::Null);
            }
        }

        Err(Signal::Error(TinyLangError::runtime(
            "match expression did not match any arm",
        )))
    }

    fn match_pattern(&self, pattern: &Pattern, value: &Value) -> Option<Option<(String, Value)>> {
        match pattern {
            Pattern::IntLit(expected) => match value {
                Value::Int(actual) if actual == expected => Some(None),
                _ => None,
            },
            Pattern::StringLit(expected) => match value {
                Value::String(actual) => {
                    let actual_str = self.heap.get_string(actual);
                    if &actual_str == expected {
                        Some(None)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            Pattern::BoolLit(expected) => match value {
                Value::Bool(actual) if actual == expected => Some(None),
                _ => None,
            },
            Pattern::Ident(name) => Some(Some((name.clone(), value.clone()))),
            Pattern::Wildcard => Some(None),
            Pattern::EnumVariant { enum_name, variant, bindings } => match value {
                Value::EnumVariant(reference) => {
                    let ev = self.heap.get_enum_variant(reference);
                    if ev.enum_name != *enum_name || ev.variant_name != *variant {
                        return None;
                    }
                    if let Some(binding_names) = bindings {
                        // Return first binding (simplified: bind whole variant for now)
                        if binding_names.is_empty() {
                            Some(None)
                        } else {
                            // Bind field values by position
                            // For simplicity, just bind whole value to first name
                            Some(Some((binding_names[0].clone(), value.clone())))
                        }
                    } else {
                        Some(None)
                    }
                }
                _ => None,
            },
        }
    }

    fn create_struct_instance(&mut self, name: &str, fields: &[(String, Expr)]) -> RuntimeResult<Value> {
        let struct_def = self.env.borrow().get_struct(name)?;
        let mut values = HashMap::new();

        for (field_name, expr) in fields {
            let value = self.evaluate_expr(expr)?;
            let expected = struct_def
                .fields
                .iter()
                .find(|(declared_name, _)| declared_name == field_name)
                .ok_or_else(|| {
                    Signal::Error(TinyLangError::runtime(format!(
                        "Struct '{}' has no field '{}'",
                        name, field_name
                    )))
                })?;

            if let Some(annotation) = &expected.1 {
                self.ensure_type_matches(annotation, &value)?;
            }
            values.insert(field_name.clone(), value);
        }

        for (field_name, _) in &struct_def.fields {
            if !values.contains_key(field_name) {
                return Err(Signal::Error(TinyLangError::runtime(format!(
                    "Struct '{}' initialization missing field '{}'",
                    name, field_name
                ))));
            }
        }

        Ok(struct_instance_value(&mut self.heap, name.to_string(), values))
    }

    fn create_enum_variant(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        fields: Option<&[(String, Expr)]>,
    ) -> RuntimeResult<Value> {
        let mut values = std::collections::HashMap::new();
        if let Some(fields) = fields {
            for (field_name, expr) in fields {
                let value = self.evaluate_expr(expr)?;
                values.insert(field_name.clone(), value);
            }
        }
        Ok(enum_variant_value(&mut self.heap, enum_name.to_string(), variant_name.to_string(), values))
    }

    fn assign_field(&mut self, object: Value, field: &str, value: Value) -> RuntimeResult<Value> {
        match object {
            Value::StructInstance(instance) => {
                let type_name = self.heap.with_struct_instance(&instance, |inst| inst.type_name.clone());
                let struct_def = self.env.borrow().get_struct(&type_name)?;
                let field_def = struct_def
                    .fields
                    .iter()
                    .find(|(name, _)| name == field)
                    .ok_or_else(|| {
                        Signal::Error(TinyLangError::runtime(format!(
                            "Struct '{}' has no field '{}'",
                            type_name, field
                        )))
                    })?
                    .clone();

                if let Some(annotation) = &field_def.1 {
                    self.ensure_type_matches(annotation, &value)?;
                }

                self.heap.with_struct_instance_mut(&instance, |inst| {
                    inst.fields.insert(field.to_string(), value);
                });
                Ok(Value::Null)
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Field assignment expects struct instance, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn assign_index(&mut self, target: Value, index: Value, value: Value) -> RuntimeResult<Value> {
        match target {
            Value::Array(items) => {
                let idx = self.expect_index(index)?;
                self.heap.with_array_mut(&items, |arr| {
                    if idx >= arr.len() {
                        Err(Signal::Error(TinyLangError::runtime(format!(
                            "Index out of bounds: array length is {}, index is {}",
                            arr.len(),
                            idx
                        ))))
                    } else {
                        arr[idx] = value;
                        Ok(Value::Null)
                    }
                })
            }
            Value::Map(items) => {
                let key = self.expect_map_key(index)?;
                self.heap.with_map_mut(&items, |m| {
                    m.insert(key, value);
                });
                Ok(Value::Null)
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index assignment expects Array or Map, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn read_index(&mut self, target: Value, index: Value) -> RuntimeResult<Value> {
        match target {
            Value::Array(items) => {
                let idx = self.expect_index(index)?;
                self.heap.with_array(&items, |arr| {
                    arr.get(idx).cloned().ok_or_else(|| {
                        Signal::Error(TinyLangError::runtime(format!(
                            "Index out of bounds: array length is {}, index is {}",
                            arr.len(),
                            idx
                        )))
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
                    None => Err(Signal::Error(TinyLangError::runtime(format!(
                        "Index out of bounds: string length is {}, index is {}",
                        chars.len(),
                        idx
                    )))),
                }
            }
            Value::Map(items) => {
                let key = self.expect_map_key(index)?;
                Ok(self.heap.with_map(&items, |m| m.get(&key).cloned().unwrap_or(Value::Null)))
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index access expects Array, String, or Map, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn read_field_or_method(&mut self, object: Value, field: &str) -> RuntimeResult<Value> {
        match object {
            Value::StructInstance(instance) => {
                let type_name = self.heap.with_struct_instance(&instance, |inst| inst.type_name.clone());
                let field_value = self.heap.with_struct_instance(&instance, |inst| inst.fields.get(field).cloned());

                if let Some(value) = field_value {
                    return Ok(value);
                }

                let method = self.env.borrow().get_method(&type_name, field)?;
                Ok(Value::BoundMethod(crate::environment::BoundMethodValue {
                    receiver: instance,
                    method,
                }))
            }
            // enum variant 也支援欄位存取（例如 v.field_name）
            Value::EnumVariant(reference) => {
                let field_value = self.heap.with_enum_variant(&reference, |ev| ev.fields.get(field).cloned());
                field_value.ok_or_else(|| {
                    let variant_name = self.heap.get_enum_variant(&reference).variant_name.clone();
                    Signal::Error(TinyLangError::runtime(format!(
                        "Enum variant '{}' has no field '{}'",
                        variant_name, field
                    )))
                })
            }
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Field access expects struct instance, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn ensure_type_matches(&self, annotation: &TypeAnnotation, value: &Value) -> RuntimeResult<()> {
        if self.value_matches_type(annotation, value) {
            Ok(())
        } else {
            Err(Signal::Error(TinyLangError::runtime(format!(
                "Expected {}, got {}",
                annotation.display_name(),
                value.type_name_for_error()
            ))))
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

    fn expect_index(&self, value: Value) -> RuntimeResult<usize> {
        match value {
            Value::Int(index) if index >= 0 => Ok(index as usize),
            Value::Int(index) => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index must be non-negative, got {}",
                index
            )))),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Index must be Int, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn expect_map_key(&self, value: Value) -> RuntimeResult<String> {
        match value {
            Value::String(key) => Ok(self.heap.get_string(&key)),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "Map key must be String, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn expect_int(&self, value: Value, label: &str) -> RuntimeResult<i64> {
        match value {
            Value::Int(number) => Ok(number),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "{label} must be Int, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn expect_string(&self, value: Value, label: &str) -> RuntimeResult<String> {
        match value {
            Value::String(text) => Ok(self.heap.get_string(&text)),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "{label} must be Str, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn expect_array(&self, value: Value, label: &str) -> RuntimeResult<Vec<Value>> {
        match value {
            Value::Array(items) => Ok(self.heap.with_array(&items, |arr| arr.clone())),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "{label} must be Array, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn expect_function_value(&self, value: Value, label: &str) -> RuntimeResult<FunctionValue> {
        match value {
            Value::Function(function) => Ok(function),
            other => Err(Signal::Error(TinyLangError::runtime(format!(
                "{label} must be function, got {}",
                other.type_name_for_error()
            )))),
        }
    }

    fn eval_unary(&self, op: UnaryOperator, value: Value) -> RuntimeResult<Value> {
        match op {
            UnaryOperator::Neg => match value {
                Value::Int(v) => Ok(Value::Int(-v)),
                other => Err(Signal::Error(TinyLangError::runtime(format!(
                    "Cannot negate {}",
                    other.type_name_for_error()
                )))),
            },
            UnaryOperator::Not => Ok(Value::Bool(!value.is_truthy())),
        }
    }

    fn eval_binary(&mut self, left: Value, op: BinaryOperator, right: Value) -> RuntimeResult<Value> {
        match op {
            BinaryOperator::Add => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::String(a), Value::String(b)) => {
                    let sa = self.heap.get_string(&a);
                    let sb = self.heap.get_string(&b);
                    let combined = sa + &sb;
                    Ok(string_value(&mut self.heap, combined))
                }
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
            BinaryOperator::Eq => Ok(Value::Bool(self.values_equal(&left, &right))),
            BinaryOperator::Ne => Ok(Value::Bool(!self.values_equal(&left, &right))),
            BinaryOperator::Lt => self.eval_int_compare(left, right, |a, b| a < b, "<"),
            BinaryOperator::Gt => self.eval_int_compare(left, right, |a, b| a > b, ">"),
            BinaryOperator::Le => self.eval_int_compare(left, right, |a, b| a <= b, "<="),
            BinaryOperator::Ge => self.eval_int_compare(left, right, |a, b| a >= b, ">="),
            BinaryOperator::And | BinaryOperator::Or => unreachable!(),
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
    env.define("typeof".into(), Value::Builtin(BuiltinFunction::TypeOf));
    env.define("input".into(), Value::Builtin(BuiltinFunction::Input));
    env.define("range".into(), Value::Builtin(BuiltinFunction::Range));
    env.define("keys".into(), Value::Builtin(BuiltinFunction::Keys));
    env.define("values".into(), Value::Builtin(BuiltinFunction::Values));
    env.define("abs".into(), Value::Builtin(BuiltinFunction::Abs));
    env.define("max".into(), Value::Builtin(BuiltinFunction::Max));
    env.define("min".into(), Value::Builtin(BuiltinFunction::Min));
    env.define("pow".into(), Value::Builtin(BuiltinFunction::Pow));
    env.define("split".into(), Value::Builtin(BuiltinFunction::Split));
    env.define("join".into(), Value::Builtin(BuiltinFunction::Join));
    env.define("trim".into(), Value::Builtin(BuiltinFunction::Trim));
    env.define("upper".into(), Value::Builtin(BuiltinFunction::Upper));
    env.define("lower".into(), Value::Builtin(BuiltinFunction::Lower));
    env.define("contains".into(), Value::Builtin(BuiltinFunction::Contains));
    env.define("replace".into(), Value::Builtin(BuiltinFunction::Replace));
    env.define("sort".into(), Value::Builtin(BuiltinFunction::Sort));
    env.define("reverse".into(), Value::Builtin(BuiltinFunction::Reverse));
    env.define("map".into(), Value::Builtin(BuiltinFunction::Map));
    env.define("filter".into(), Value::Builtin(BuiltinFunction::Filter));
    env.define("reduce".into(), Value::Builtin(BuiltinFunction::Reduce));
    env.define("find".into(), Value::Builtin(BuiltinFunction::Find));
    env.define("assert".into(), Value::Builtin(BuiltinFunction::Assert));
}

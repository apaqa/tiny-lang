//! tiny-lang 靜態型別檢查器
use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinaryOperator, Expr, InterfaceMethod, MatchArm, Program, Statement, TypeAnnotation, UnaryOperator,
};
use crate::error::TinyLangError;

#[derive(Debug, Clone)]
struct FuncSig {
    params: Vec<(String, Option<TypeAnnotation>)>,
    return_type: Option<TypeAnnotation>,
}

pub struct TypeChecker {
    type_env: Vec<HashMap<String, TypeAnnotation>>,
    func_env: HashMap<String, FuncSig>,
    method_env: HashMap<String, HashMap<String, FuncSig>>,
    struct_env: HashSet<String>,
    interface_env: HashMap<String, Vec<InterfaceMethod>>,
    impl_env: HashMap<String, HashSet<String>>,
    current_return_type: Option<TypeAnnotation>,
    pub errors: Vec<TinyLangError>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut interface_env = HashMap::new();
        // 內建 Iterator 介面，供 for/in 與介面型別標註共用。
        interface_env.insert(
            "Iterator".into(),
            vec![InterfaceMethod {
                name: "next".into(),
                params: vec![("self".into(), None)],
                return_type: Some(TypeAnnotation::Any),
            }],
        );

        Self {
            type_env: vec![HashMap::new()],
            func_env: HashMap::new(),
            method_env: HashMap::new(),
            struct_env: HashSet::new(),
            interface_env,
            impl_env: HashMap::new(),
            current_return_type: None,
            errors: Vec::new(),
        }
    }

    pub fn check_program(&mut self, program: &Program) {
        self.collect_declarations(program);
        self.validate_impl_blocks(program);
        for stmt in program {
            self.check_statement(stmt);
        }
    }

    fn collect_declarations(&mut self, program: &Program) {
        for stmt in program {
            match stmt {
                Statement::StructDecl { name, .. } => {
                    self.struct_env.insert(name.clone());
                }
                Statement::InterfaceDecl { name, methods } => {
                    self.interface_env.insert(name.clone(), methods.clone());
                }
                Statement::FnDecl {
                    name,
                    params,
                    return_type,
                    ..
                } => {
                    self.func_env.insert(
                        name.clone(),
                        FuncSig {
                            params: params.clone(),
                            return_type: return_type.clone(),
                        },
                    );
                }
                Statement::MethodDecl {
                    struct_name,
                    method_name,
                    params,
                    return_type,
                    ..
                } => {
                    let method_params = strip_self_param(params);
                    self.method_env.entry(struct_name.clone()).or_default().insert(
                        method_name.clone(),
                        FuncSig {
                            params: method_params,
                            return_type: return_type.clone(),
                        },
                    );
                }
                Statement::ImplInterface {
                    struct_name, methods, ..
                } => {
                    let mut collected_methods = Vec::new();
                    for method in methods {
                        if let Statement::FnDecl {
                            name,
                            params,
                            return_type,
                            ..
                        } = method
                        {
                            let Some((receiver, method_params)) = params.split_first() else {
                                self.add_error(format!(
                                    "impl method '{}.{}' must declare self as the first parameter",
                                    struct_name, name
                                ));
                                continue;
                            };
                            if receiver.0 != "self" {
                                self.add_error(format!(
                                    "impl method '{}.{}' must declare self as the first parameter",
                                    struct_name, name
                                ));
                                continue;
                            }
                            collected_methods.push((
                                name.clone(),
                                FuncSig {
                                    params: method_params.to_vec(),
                                    return_type: return_type.clone(),
                                },
                            ));
                        }
                    }
                    let method_map = self.method_env.entry(struct_name.clone()).or_default();
                    for (name, sig) in collected_methods {
                        method_map.insert(name, sig);
                    }
                }
                _ => {}
            }
        }
    }

    fn validate_impl_blocks(&mut self, program: &Program) {
        for stmt in program {
            let Statement::ImplInterface {
                interface_name,
                struct_name,
                ..
            } = stmt
            else {
                continue;
            };

            if !self.struct_env.contains(struct_name) {
                self.add_error(format!("Struct '{}' not defined", struct_name));
                continue;
            }

            let Some(interface_methods) = self.interface_env.get(interface_name).cloned() else {
                self.add_error(format!("Interface '{}' not defined", interface_name));
                continue;
            };

            for method in interface_methods {
                let Some((receiver, expected_params)) = method.params.split_first() else {
                    self.add_error(format!(
                        "interface '{}.{}' must declare self as the first parameter",
                        interface_name, method.name
                    ));
                    continue;
                };
                if receiver.0 != "self" {
                    self.add_error(format!(
                        "interface '{}.{}' must declare self as the first parameter",
                        interface_name, method.name
                    ));
                    continue;
                }

                let Some(actual) = self
                    .method_env
                    .get(struct_name)
                    .and_then(|methods| methods.get(&method.name))
                    .cloned()
                else {
                    self.add_error(format!(
                        "Struct '{}' does not fully implement interface '{}': missing method '{}'",
                        struct_name, interface_name, method.name
                    ));
                    continue;
                };

                if actual.params != expected_params || actual.return_type != method.return_type {
                    self.add_error(format!(
                        "Method '{}.{}' does not match interface '{}'",
                        struct_name, method.name, interface_name
                    ));
                }
            }

            self.impl_env
                .entry(struct_name.clone())
                .or_default()
                .insert(interface_name.clone());
        }
    }

    fn push_scope(&mut self) {
        self.type_env.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.type_env.pop();
    }

    fn declare_var(&mut self, name: &str, ty: TypeAnnotation) {
        if let Some(scope) = self.type_env.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_var(&self, name: &str) -> Option<&TypeAnnotation> {
        for scope in self.type_env.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    fn add_error(&mut self, message: String) {
        self.errors.push(TinyLangError::type_check(message));
    }

    fn infer_type(&self, expr: &Expr) -> Option<TypeAnnotation> {
        match expr {
            Expr::IntLit(_) => Some(TypeAnnotation::Int),
            Expr::StringLit(_) => Some(TypeAnnotation::Str),
            Expr::BoolLit(_) => Some(TypeAnnotation::Bool),
            Expr::NullLit => None,
            Expr::Ident(name) => self.lookup_var(name).cloned(),
            Expr::StructInit { name, .. } => Some(TypeAnnotation::Named(name.clone())),
            Expr::BinaryOp { op, left, right } => {
                let lt = self.infer_type(left);
                let rt = self.infer_type(right);
                self.infer_binop_type(*op, lt.as_ref(), rt.as_ref())
            }
            Expr::UnaryOp { op, operand } => {
                let operand_ty = self.infer_type(operand);
                match op {
                    UnaryOperator::Neg if matches!(operand_ty, Some(TypeAnnotation::Int)) => {
                        Some(TypeAnnotation::Int)
                    }
                    UnaryOperator::Not => Some(TypeAnnotation::Bool),
                    _ => None,
                }
            }
            Expr::FnCall { callee, .. } => match callee.as_ref() {
                Expr::Ident(name) => self.func_env.get(name).and_then(|sig| sig.return_type.clone()),
                Expr::FieldAccess { object, field } => {
                    let object_ty = self.infer_type(object)?;
                    let TypeAnnotation::Named(struct_name) = object_ty else {
                        return None;
                    };
                    self.method_env
                        .get(&struct_name)
                        .and_then(|methods| methods.get(field))
                        .and_then(|sig| sig.return_type.clone())
                }
                _ => None,
            },
            Expr::ArrayLit(items) => {
                let first_ty = items.first().and_then(|item| self.infer_type(item))?;
                Some(TypeAnnotation::ArrayOf(Box::new(first_ty)))
            }
            _ => None,
        }
    }

    fn infer_binop_type(
        &self,
        op: BinaryOperator,
        lt: Option<&TypeAnnotation>,
        rt: Option<&TypeAnnotation>,
    ) -> Option<TypeAnnotation> {
        match op {
            BinaryOperator::Add => match (lt, rt) {
                (Some(TypeAnnotation::Int), Some(TypeAnnotation::Int)) => Some(TypeAnnotation::Int),
                (Some(TypeAnnotation::Str), Some(TypeAnnotation::Str)) => Some(TypeAnnotation::Str),
                _ => None,
            },
            BinaryOperator::Sub
            | BinaryOperator::Mul
            | BinaryOperator::Div
            | BinaryOperator::Mod => match (lt, rt) {
                (Some(TypeAnnotation::Int), Some(TypeAnnotation::Int)) => Some(TypeAnnotation::Int),
                _ => None,
            },
            BinaryOperator::Eq
            | BinaryOperator::Ne
            | BinaryOperator::Lt
            | BinaryOperator::Gt
            | BinaryOperator::Le
            | BinaryOperator::Ge
            | BinaryOperator::And
            | BinaryOperator::Or => Some(TypeAnnotation::Bool),
        }
    }

    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::LetDecl {
                name,
                type_annotation,
                value,
            } => {
                let val_ty = self.infer_type(value);
                if let Some(annotation) = type_annotation {
                    self.check_compatible(
                        annotation,
                        val_ty.clone(),
                        &format!("let {name}: expected {}", annotation.display_name()),
                    );
                    self.declare_var(name, annotation.clone());
                } else if let Some(inferred) = val_ty {
                    self.declare_var(name, inferred);
                }
                self.check_expr(value);
            }
            Statement::Assignment { name, value } => {
                let val_ty = self.infer_type(value);
                if let Some(declared) = self.lookup_var(name).cloned() {
                    self.check_compatible(
                        &declared,
                        val_ty,
                        &format!("assignment to {name}: expected {}", declared.display_name()),
                    );
                }
                self.check_expr(value);
            }
            Statement::FnDecl {
                name,
                params,
                return_type,
                body,
            } => {
                self.func_env.insert(
                    name.clone(),
                    FuncSig {
                        params: params.clone(),
                        return_type: return_type.clone(),
                    },
                );
                self.push_scope();
                for (param_name, param_type) in params {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, ty.clone());
                    }
                }
                let outer = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                for s in body {
                    self.check_statement(s);
                }
                self.current_return_type = outer;
                self.pop_scope();
            }
            Statement::MethodDecl {
                struct_name,
                params,
                body,
                return_type,
                ..
            } => {
                self.push_scope();
                // method 的 self 由 runtime 注入，型別檢查這裡直接提供 struct 型別。
                self.declare_var("self", TypeAnnotation::Named(struct_name.clone()));
                for (param_name, param_type) in params.iter().filter(|(name, _)| name != "self") {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, ty.clone());
                    }
                }
                let outer = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                for s in body {
                    self.check_statement(s);
                }
                self.current_return_type = outer;
                self.pop_scope();
            }
            Statement::InterfaceDecl { .. } => {}
            Statement::ImplInterface {
                struct_name, methods, ..
            } => {
                for method in methods {
                    let Statement::FnDecl {
                        params,
                        return_type,
                        body,
                        ..
                    } = method
                    else {
                        continue;
                    };
                    self.push_scope();
                    self.declare_var("self", TypeAnnotation::Named(struct_name.clone()));
                    for (param_name, param_type) in params.iter().skip(1) {
                        if let Some(ty) = param_type {
                            self.declare_var(param_name, ty.clone());
                        }
                    }
                    let outer = self.current_return_type.take();
                    self.current_return_type = return_type.clone();
                    for s in body {
                        self.check_statement(s);
                    }
                    self.current_return_type = outer;
                    self.pop_scope();
                }
            }
            Statement::Return(expr) => {
                if let Some(expected) = self.current_return_type.clone() {
                    self.check_compatible(
                        &expected,
                        self.infer_type(expr),
                        &format!("return: expected {}", expected.display_name()),
                    );
                }
                self.check_expr(expr);
            }
            Statement::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                self.check_expr(condition);
                self.push_scope();
                for s in then_body {
                    self.check_statement(s);
                }
                self.pop_scope();
                if let Some(else_body) = else_body {
                    self.push_scope();
                    for s in else_body {
                        self.check_statement(s);
                    }
                    self.pop_scope();
                }
            }
            Statement::While { condition, body } => {
                self.check_expr(condition);
                self.push_scope();
                for s in body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }
            Statement::ForLoop {
                variable,
                iterable,
                body,
            } => {
                self.check_expr(iterable);
                self.push_scope();
                match self.infer_type(iterable) {
                    Some(TypeAnnotation::ArrayOf(elem_ty)) => self.declare_var(variable, *elem_ty),
                    Some(TypeAnnotation::Named(struct_name))
                        if self.struct_implements_interface(&struct_name, "Iterator") =>
                    {
                        self.declare_var(variable, TypeAnnotation::Any);
                    }
                    Some(TypeAnnotation::Named(struct_name)) => {
                        self.add_error(format!(
                            "Struct '{}' cannot be used in for/in because it does not implement Iterator",
                            struct_name
                        ));
                    }
                    _ => {}
                }
                for s in body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }
            Statement::Match { expr, arms } => {
                self.check_expr(expr);
                for arm in arms {
                    self.check_match_arm(arm);
                }
            }
            Statement::TryCatch {
                try_body,
                catch_var,
                catch_body,
            } => {
                self.push_scope();
                for s in try_body {
                    self.check_statement(s);
                }
                self.pop_scope();
                self.push_scope();
                self.declare_var(catch_var, TypeAnnotation::Str);
                for s in catch_body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }
            Statement::Print(expr) | Statement::ExprStatement(expr) => self.check_expr(expr),
            Statement::IndexAssignment { target, index, value } => {
                self.check_expr(target);
                self.check_expr(index);
                self.check_expr(value);
            }
            Statement::FieldAssignment { object, value, .. } => {
                self.check_expr(object);
                self.check_expr(value);
            }
            Statement::StructDecl { .. }
            | Statement::EnumDecl { .. }
            | Statement::Import { .. }
            | Statement::Break
            | Statement::Continue => {}
        }
    }

    fn check_match_arm(&mut self, arm: &MatchArm) {
        self.push_scope();
        for s in &arm.body {
            self.check_statement(s);
        }
        self.pop_scope();
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::BinaryOp { op, left, right } => {
                self.check_expr(left);
                self.check_expr(right);
                self.check_binop(*op, self.infer_type(left), self.infer_type(right));
            }
            Expr::UnaryOp { op, operand } => {
                self.check_expr(operand);
                self.check_unaryop(*op, self.infer_type(operand));
            }
            Expr::FnCall { callee, args } => {
                self.check_expr(callee);
                for arg in args {
                    self.check_expr(arg);
                }
                match callee.as_ref() {
                    Expr::Ident(name) => {
                        if let Some(sig) = self.func_env.get(name).cloned() {
                            self.check_call_args(name, args, &sig);
                        }
                    }
                    Expr::FieldAccess { object, field } => {
                        if let Some(TypeAnnotation::Named(struct_name)) = self.infer_type(object) {
                            if let Some(sig) = self
                                .method_env
                                .get(&struct_name)
                                .and_then(|methods| methods.get(field))
                                .cloned()
                            {
                                self.check_call_args(&format!("{struct_name}.{field}"), args, &sig);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Expr::ArrayLit(items) => {
                for item in items {
                    self.check_expr(item);
                }
            }
            Expr::MapLit(pairs) => {
                for (key, value) in pairs {
                    self.check_expr(key);
                    self.check_expr(value);
                }
            }
            Expr::IndexAccess { target, index } => {
                self.check_expr(target);
                self.check_expr(index);
            }
            Expr::FieldAccess { object, .. } => self.check_expr(object),
            Expr::StructInit { fields, .. } => {
                for (_, value) in fields {
                    self.check_expr(value);
                }
            }
            Expr::EnumVariant { fields, .. } => {
                if let Some(fields) = fields {
                    for (_, value) in fields {
                        self.check_expr(value);
                    }
                }
            }
            Expr::Lambda { body, .. } => {
                self.push_scope();
                for s in body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }
            Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) | Expr::NullLit | Expr::Ident(_) => {}
        }
    }

    fn check_compatible(
        &mut self,
        expected: &TypeAnnotation,
        actual: Option<TypeAnnotation>,
        context_prefix: &str,
    ) {
        if let Some(actual_ty) = actual {
            if !self.types_compatible(expected, &actual_ty) {
                self.add_error(format!(
                    "{} but got {}",
                    context_prefix,
                    actual_ty.display_name()
                ));
            }
        }
    }

    fn check_binop(
        &mut self,
        op: BinaryOperator,
        lt: Option<TypeAnnotation>,
        rt: Option<TypeAnnotation>,
    ) {
        let (Some(lt), Some(rt)) = (&lt, &rt) else {
            return;
        };

        match op {
            BinaryOperator::Add => {
                if !matches!(
                    (lt, rt),
                    (TypeAnnotation::Int, TypeAnnotation::Int)
                        | (TypeAnnotation::Str, TypeAnnotation::Str)
                ) {
                    self.add_error(format!(
                        "operator '+' does not support {} and {}",
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::Sub | BinaryOperator::Mul | BinaryOperator::Div | BinaryOperator::Mod => {
                if !matches!((lt, rt), (TypeAnnotation::Int, TypeAnnotation::Int)) {
                    self.add_error(format!(
                        "operator '{}' requires int operands, got {} and {}",
                        binop_symbol(op),
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::Lt | BinaryOperator::Gt | BinaryOperator::Le | BinaryOperator::Ge => {
                if !matches!(
                    (lt, rt),
                    (TypeAnnotation::Int, TypeAnnotation::Int)
                        | (TypeAnnotation::Str, TypeAnnotation::Str)
                ) {
                    self.add_error(format!(
                        "operator '{}' cannot compare {} and {}",
                        binop_symbol(op),
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::And | BinaryOperator::Or => {
                if !matches!(lt, TypeAnnotation::Bool) || !matches!(rt, TypeAnnotation::Bool) {
                    self.add_error(format!(
                        "operator '{}' requires bool operands, got {} and {}",
                        binop_symbol(op),
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::Eq | BinaryOperator::Ne => {}
        }
    }

    fn check_unaryop(&mut self, op: UnaryOperator, operand_ty: Option<TypeAnnotation>) {
        let Some(ty) = operand_ty else {
            return;
        };
        match op {
            UnaryOperator::Neg => {
                if !matches!(ty, TypeAnnotation::Int) {
                    self.add_error(format!("operator '-' requires int, got {}", ty.display_name()));
                }
            }
            UnaryOperator::Not => {
                if !matches!(ty, TypeAnnotation::Bool) {
                    self.add_error(format!("operator '!' requires bool, got {}", ty.display_name()));
                }
            }
        }
    }

    fn check_call_args(&mut self, fn_name: &str, args: &[Expr], sig: &FuncSig) {
        for (index, (arg, (param_name, param_type))) in args.iter().zip(sig.params.iter()).enumerate() {
            let Some(expected) = param_type else {
                continue;
            };
            let Some(actual) = self.infer_type(arg) else {
                continue;
            };
            if !self.types_compatible(expected, &actual) {
                self.add_error(format!(
                    "call '{}': argument {} ('{}') expects {}, got {}",
                    fn_name,
                    index + 1,
                    param_name,
                    expected.display_name(),
                    actual.display_name()
                ));
            }
        }
    }

    fn types_compatible(&self, expected: &TypeAnnotation, actual: &TypeAnnotation) -> bool {
        match (expected, actual) {
            (TypeAnnotation::Any, _) | (_, TypeAnnotation::Any) => true,
            (TypeAnnotation::Int, TypeAnnotation::Int) => true,
            (TypeAnnotation::Str, TypeAnnotation::Str) => true,
            (TypeAnnotation::Bool, TypeAnnotation::Bool) => true,
            (TypeAnnotation::ArrayOf(expected), TypeAnnotation::ArrayOf(actual)) => {
                self.types_compatible(expected, actual)
            }
            (TypeAnnotation::MapOf(expected), TypeAnnotation::MapOf(actual)) => {
                self.types_compatible(expected, actual)
            }
            (TypeAnnotation::Named(expected_name), TypeAnnotation::Named(actual_name)) => {
                expected_name == actual_name
                    || (self.interface_env.contains_key(expected_name)
                        && self.struct_implements_interface(actual_name, expected_name))
            }
            _ => false,
        }
    }

    fn struct_implements_interface(&self, struct_name: &str, interface_name: &str) -> bool {
        if interface_name == "Iterator" {
            return self
                .method_env
                .get(struct_name)
                .is_some_and(|methods| methods.contains_key("next"));
        }

        self.impl_env
            .get(struct_name)
            .is_some_and(|interfaces| interfaces.contains(interface_name))
    }
}

pub fn types_compatible(expected: &TypeAnnotation, actual: &TypeAnnotation) -> bool {
    match (expected, actual) {
        (TypeAnnotation::Any, _) | (_, TypeAnnotation::Any) => true,
        (TypeAnnotation::Int, TypeAnnotation::Int) => true,
        (TypeAnnotation::Str, TypeAnnotation::Str) => true,
        (TypeAnnotation::Bool, TypeAnnotation::Bool) => true,
        (TypeAnnotation::ArrayOf(expected), TypeAnnotation::ArrayOf(actual)) => {
            types_compatible(expected, actual)
        }
        (TypeAnnotation::MapOf(expected), TypeAnnotation::MapOf(actual)) => {
            types_compatible(expected, actual)
        }
        (TypeAnnotation::Named(expected), TypeAnnotation::Named(actual)) => expected == actual,
        _ => false,
    }
}

fn strip_self_param(params: &[(String, Option<TypeAnnotation>)]) -> Vec<(String, Option<TypeAnnotation>)> {
    if let Some((first, rest)) = params.split_first() {
        if first.0 == "self" {
            return rest.to_vec();
        }
    }
    params.to_vec()
}

fn binop_symbol(op: BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Add => "+",
        BinaryOperator::Sub => "-",
        BinaryOperator::Mul => "*",
        BinaryOperator::Div => "/",
        BinaryOperator::Mod => "%",
        BinaryOperator::Eq => "==",
        BinaryOperator::Ne => "!=",
        BinaryOperator::Lt => "<",
        BinaryOperator::Gt => ">",
        BinaryOperator::Le => "<=",
        BinaryOperator::Ge => ">=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
    }
}

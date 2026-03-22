//! tiny-lang 型別檢查器
use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinaryOperator, Expr, InterfaceMethod, MatchArm, Pattern, Program, Statement, TypeAnnotation, UnaryOperator,
};
use crate::error::TinyLangError;

#[derive(Debug, Clone)]
struct FuncSig {
    type_params: Vec<String>,
    params: Vec<(String, Option<TypeAnnotation>)>,
    return_type: Option<TypeAnnotation>,
}

#[derive(Debug, Clone)]
struct EnumSig {
    type_params: Vec<String>,
    variants: HashMap<String, EnumVariantSig>,
}

#[derive(Debug, Clone)]
struct EnumVariantSig {
    fields: Vec<(String, Option<TypeAnnotation>)>,
}

pub struct TypeChecker {
    type_env: Vec<HashMap<String, TypeAnnotation>>,
    func_env: HashMap<String, FuncSig>,
    method_env: HashMap<String, HashMap<String, FuncSig>>,
    struct_env: HashSet<String>,
    enum_env: HashMap<String, EnumSig>,
    interface_env: HashMap<String, Vec<InterfaceMethod>>,
    impl_env: HashMap<String, HashSet<String>>,
    current_return_type: Option<TypeAnnotation>,
    // 中文註解：記錄目前函式作用域可用的泛型參數，讓 T/U 這些名字能當作型別變數使用。
    generic_param_scopes: Vec<HashSet<String>>,
    pub errors: Vec<TinyLangError>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            type_env: vec![HashMap::new()],
            func_env: HashMap::new(),
            method_env: HashMap::new(),
            struct_env: HashSet::new(),
            enum_env: HashMap::new(),
            interface_env: HashMap::new(),
            impl_env: HashMap::new(),
            current_return_type: None,
            generic_param_scopes: Vec::new(),
            errors: Vec::new(),
        };
        checker.install_builtin_interfaces();
        checker.install_builtin_enums();
        checker
    }

    pub fn check_program(&mut self, program: &Program) {
        self.collect_declarations(program);
        self.validate_impl_blocks(program);
        for stmt in program {
            self.check_statement(stmt);
        }
    }

    fn install_builtin_interfaces(&mut self) {
        self.interface_env.insert(
            "Iterator".into(),
            vec![InterfaceMethod {
                name: "next".into(),
                params: vec![("self".into(), None)],
                return_type: Some(TypeAnnotation::Any),
            }],
        );
    }

    fn install_builtin_enums(&mut self) {
        self.enum_env.insert(
            "Option".into(),
            EnumSig {
                type_params: vec!["T".into()],
                variants: HashMap::from([
                    (
                        "Some".into(),
                        EnumVariantSig {
                            fields: vec![("0".into(), Some(TypeAnnotation::Named("T".into())))],
                        },
                    ),
                    ("None".into(), EnumVariantSig { fields: vec![] }),
                ]),
            },
        );
        self.enum_env.insert(
            "Result".into(),
            EnumSig {
                type_params: vec!["T".into(), "E".into()],
                variants: HashMap::from([
                    (
                        "Ok".into(),
                        EnumVariantSig {
                            fields: vec![("0".into(), Some(TypeAnnotation::Named("T".into())))],
                        },
                    ),
                    (
                        "Err".into(),
                        EnumVariantSig {
                            fields: vec![("0".into(), Some(TypeAnnotation::Named("E".into())))],
                        },
                    ),
                ]),
            },
        );
    }

    fn collect_declarations(&mut self, program: &Program) {
        for stmt in program {
            match stmt {
                Statement::StructDecl { name, .. } => {
                    self.struct_env.insert(name.clone());
                }
                Statement::EnumDecl { name, variants } => {
                    self.enum_env.insert(
                        name.clone(),
                        EnumSig {
                            type_params: Vec::new(),
                            variants: variants
                                .iter()
                                .map(|variant| {
                                    (
                                        variant.name.clone(),
                                        EnumVariantSig {
                                            fields: variant.fields.clone().unwrap_or_default(),
                                        },
                                    )
                                })
                                .collect(),
                        },
                    );
                }
                Statement::InterfaceDecl { name, methods } => {
                    self.interface_env.insert(name.clone(), methods.clone());
                }
                Statement::FnDecl {
                    name,
                    type_params,
                    params,
                    return_type,
                    ..
                } => {
                    self.func_env.insert(
                        name.clone(),
                        FuncSig {
                            type_params: type_params.clone(),
                            params: params.clone(),
                            return_type: return_type.clone(),
                        },
                    );
                }
                Statement::AsyncFnDecl {
                    name,
                    type_params,
                    params,
                    return_type,
                    ..
                } => {
                    // 中文註解：async 函式宣告與普通函式同樣收集進 func_env。
                    self.func_env.insert(
                        name.clone(),
                        FuncSig {
                            type_params: type_params.clone(),
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
                            type_params: Vec::new(),
                            params: method_params,
                            return_type: return_type.clone(),
                        },
                    );
                }
                Statement::ImplInterface {
                    struct_name, methods, ..
                } => {
                    let mut local_errors: Vec<String> = Vec::new();
                    let method_map = self.method_env.entry(struct_name.clone()).or_default();
                    for method in methods {
                        if let Statement::FnDecl {
                            name,
                            params,
                            return_type,
                            ..
                        } = method
                        {
                            let Some((receiver, method_params)) = params.split_first() else {
                                local_errors.push(format!(
                                    "impl method '{}.{}' must declare self as the first parameter",
                                    struct_name, name
                                ));
                                continue;
                            };
                            if receiver.0 != "self" {
                                local_errors.push(format!(
                                    "impl method '{}.{}' must declare self as the first parameter",
                                    struct_name, name
                                ));
                                continue;
                            }
                            method_map.insert(
                                name.clone(),
                                FuncSig {
                                    type_params: Vec::new(),
                                    params: method_params.to_vec(),
                                    return_type: return_type.clone(),
                                },
                            );
                        }
                    }
                    for err in local_errors {
                        self.add_error(err);
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

    fn push_generic_scope(&mut self, params: &[String]) {
        self.generic_param_scopes
            .push(params.iter().cloned().collect::<HashSet<_>>());
    }

    fn pop_generic_scope(&mut self) {
        self.generic_param_scopes.pop();
    }

    fn is_generic_param(&self, name: &str) -> bool {
        self.generic_param_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
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

    fn normalize_type(&self, ty: &TypeAnnotation) -> TypeAnnotation {
        match ty {
            TypeAnnotation::Generic { name, type_params } if name == "Array" && type_params.len() == 1 => {
                TypeAnnotation::ArrayOf(Box::new(self.normalize_type(&type_params[0])))
            }
            TypeAnnotation::Generic { name, type_params } if name == "Map" && type_params.len() == 1 => {
                TypeAnnotation::MapOf(Box::new(self.normalize_type(&type_params[0])))
            }
            TypeAnnotation::ArrayOf(inner) => TypeAnnotation::ArrayOf(Box::new(self.normalize_type(inner))),
            TypeAnnotation::MapOf(inner) => TypeAnnotation::MapOf(Box::new(self.normalize_type(inner))),
            TypeAnnotation::Generic { name, type_params } => TypeAnnotation::Generic {
                name: name.clone(),
                type_params: type_params.iter().map(|param| self.normalize_type(param)).collect(),
            },
            other => other.clone(),
        }
    }

    fn infer_type(&self, expr: &Expr) -> Option<TypeAnnotation> {
        match expr {
            Expr::IntLit(_) => Some(TypeAnnotation::Int),
            Expr::StringLit(_) => Some(TypeAnnotation::Str),
            Expr::BoolLit(_) => Some(TypeAnnotation::Bool),
            Expr::NullLit => None,
            Expr::Ident(name) => self.lookup_var(name).cloned(),
            Expr::StructInit { name, .. } => Some(TypeAnnotation::Named(name.clone())),
            Expr::EnumVariant {
                enum_name,
                variant,
                fields,
            } => self.infer_enum_variant_type(enum_name, variant, fields.as_deref()),
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
            Expr::FnCall { callee, args } => match callee.as_ref() {
                Expr::Ident(name) => self
                    .func_env
                    .get(name)
                    .and_then(|sig| self.infer_call_return_type(sig, args)),
                Expr::FieldAccess { object, field } => {
                    let object_ty = self.infer_type(object)?;
                    let TypeAnnotation::Named(struct_name) = object_ty else {
                        return None;
                    };
                    self.method_env
                        .get(&struct_name)
                        .and_then(|methods| methods.get(field))
                        .and_then(|sig| self.infer_call_return_type(sig, args))
                }
                _ => None,
            },
            Expr::ArrayLit(items) => {
                let first_ty = items.first().and_then(|item| self.infer_type(item))?;
                Some(TypeAnnotation::ArrayOf(Box::new(first_ty)))
            }
            Expr::IndexAccess { target, .. } => match self.infer_type(target)? {
                TypeAnnotation::ArrayOf(inner) => Some(*inner),
                TypeAnnotation::Str => Some(TypeAnnotation::Str),
                _ => None,
            },
            Expr::FieldAccess { .. } | Expr::MapLit(_) | Expr::Lambda { .. } => None,
            Expr::Await { .. } => None,
        }
    }

    fn infer_enum_variant_type(
        &self,
        enum_name: &str,
        variant_name: &str,
        fields: Option<&[(String, Expr)]>,
    ) -> Option<TypeAnnotation> {
        let enum_sig = self.enum_env.get(enum_name)?;
        let variant_sig = enum_sig.variants.get(variant_name)?;
        let mut bindings = HashMap::new();
        for ((_, actual_expr), (_, expected_ty)) in fields.unwrap_or(&[]).iter().zip(variant_sig.fields.iter()) {
            let actual_ty = self.infer_type(actual_expr)?;
            if let Some(expected_ty) = expected_ty {
                self.collect_type_bindings_with_generics(
                    &enum_sig.type_params,
                    expected_ty,
                    &actual_ty,
                    &mut bindings,
                );
            }
        }

        if enum_sig.type_params.is_empty() {
            Some(TypeAnnotation::Named(enum_name.into()))
        } else {
            Some(TypeAnnotation::Generic {
                name: enum_name.into(),
                type_params: enum_sig
                    .type_params
                    .iter()
                    .map(|param| bindings.get(param).cloned().unwrap_or(TypeAnnotation::Any))
                    .collect(),
            })
        }
    }

    fn infer_call_return_type(&self, sig: &FuncSig, args: &[Expr]) -> Option<TypeAnnotation> {
        let mut bindings = HashMap::new();
        for (arg, (_, annotation)) in args.iter().zip(sig.params.iter()) {
            let Some(param_ty) = annotation else {
                continue;
            };
            let actual_ty = self.infer_type(arg)?;
            self.collect_type_bindings_with_generics(&sig.type_params, param_ty, &actual_ty, &mut bindings);
        }
        sig.return_type
            .as_ref()
            .map(|return_ty| self.substitute_type_vars_with_generics(&sig.type_params, return_ty, &bindings))
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
                self.check_expr(value);
                let val_ty = self.infer_type(value);
                if let Some(annotation) = type_annotation {
                    self.check_compatible(
                        annotation,
                        val_ty.clone(),
                        &format!("let {name}: expected {}", annotation.display_name()),
                    );
                    self.declare_var(name, self.normalize_type(annotation));
                } else if let Some(inferred) = val_ty {
                    self.declare_var(name, self.normalize_type(&inferred));
                }
            }
            Statement::Assignment { name, value } => {
                self.check_expr(value);
                let val_ty = self.infer_type(value);
                if let Some(declared) = self.lookup_var(name).cloned() {
                    self.check_compatible(
                        &declared,
                        val_ty,
                        &format!("assignment to {name}: expected {}", declared.display_name()),
                    );
                }
            }
            Statement::FnDecl {
                name,
                type_params,
                params,
                return_type,
                body,
            } => {
                self.func_env.insert(
                    name.clone(),
                    FuncSig {
                        type_params: type_params.clone(),
                        params: params.clone(),
                        return_type: return_type.clone(),
                    },
                );
                self.push_scope();
                self.push_generic_scope(type_params);
                for (param_name, param_type) in params {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, self.normalize_type(ty));
                    }
                }
                let outer = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                for s in body {
                    self.check_statement(s);
                }
                self.current_return_type = outer;
                self.pop_generic_scope();
                self.pop_scope();
            }
            Statement::AsyncFnDecl {
                name: _,
                type_params,
                params,
                return_type,
                body,
            } => {
                // 中文註解：async 函式的型別檢查與普通函式相同。
                self.push_scope();
                self.push_generic_scope(type_params);
                for (param_name, param_type) in params {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, self.normalize_type(ty));
                    }
                }
                let outer = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                for s in body {
                    self.check_statement(s);
                }
                self.current_return_type = outer;
                self.pop_generic_scope();
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
                self.declare_var("self", TypeAnnotation::Named(struct_name.clone()));
                for (param_name, param_type) in params.iter().filter(|(name, _)| name != "self") {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, self.normalize_type(ty));
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
            Statement::InterfaceDecl { .. } | Statement::EnumDecl { .. } | Statement::Import { .. } => {}
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
                            self.declare_var(param_name, self.normalize_type(ty));
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
                self.check_expr(expr);
                if let Some(expected) = self.current_return_type.clone() {
                    self.check_compatible(
                        &expected,
                        self.infer_type(expr),
                        &format!("return: expected {}", expected.display_name()),
                    );
                }
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
                let matched_ty = self.infer_type(expr);
                for arm in arms {
                    self.check_match_arm(arm, matched_ty.clone());
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
            Statement::StructDecl { .. } | Statement::Break | Statement::Continue => {}
        }
    }

    fn check_match_arm(&mut self, arm: &MatchArm, matched_ty: Option<TypeAnnotation>) {
        self.push_scope();
        self.bind_pattern(&arm.pattern, matched_ty);
        for s in &arm.body {
            self.check_statement(s);
        }
        self.pop_scope();
    }

    fn bind_pattern(&mut self, pattern: &Pattern, matched_ty: Option<TypeAnnotation>) {
        match pattern {
            Pattern::Ident(name) => {
                if let Some(ty) = matched_ty {
                    self.declare_var(name, ty);
                }
            }
            Pattern::EnumVariant {
                enum_name,
                variant,
                bindings,
            } => {
                let Some(binding_names) = bindings else {
                    return;
                };
                let Some(enum_sig) = self.enum_env.get(enum_name).cloned() else {
                    return;
                };
                let Some(variant_sig) = enum_sig.variants.get(variant).cloned() else {
                    return;
                };
                let mut type_bindings = HashMap::new();
                if let Some(actual_ty) = matched_ty {
                    self.collect_type_bindings_with_generics(
                        &enum_sig.type_params,
                        &self.enum_type_from_sig(enum_name, &enum_sig),
                        &actual_ty,
                        &mut type_bindings,
                    );
                }
                for (binding_name, (_, field_ty)) in binding_names.iter().zip(variant_sig.fields.iter()) {
                    if let Some(field_ty) = field_ty {
                        self.declare_var(
                            binding_name,
                            self.substitute_type_vars_with_generics(&enum_sig.type_params, field_ty, &type_bindings),
                        );
                    } else {
                        self.declare_var(binding_name, TypeAnnotation::Any);
                    }
                }
            }
            Pattern::IntLit(_) | Pattern::StringLit(_) | Pattern::BoolLit(_) | Pattern::Wildcard => {}
        }
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
                if let Some(first_ty) = items.first().and_then(|item| self.infer_type(item)) {
                    for item in items.iter().skip(1) {
                        let item_ty = self.infer_type(item);
                        self.check_compatible(&first_ty, item_ty, "array literal items must have the same type");
                    }
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
            Expr::EnumVariant {
                enum_name,
                variant,
                fields,
            } => {
                if let Some(fields) = fields {
                    for (_, value) in fields {
                        self.check_expr(value);
                    }
                }
                self.check_enum_variant_constructor(enum_name, variant, fields.as_deref());
            }
            Expr::Lambda { body, .. } => {
                self.push_scope();
                for s in body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }
            Expr::Await { expr } => self.check_expr(expr),
            Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) | Expr::NullLit | Expr::Ident(_) => {}
        }
    }

    fn check_enum_variant_constructor(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        fields: Option<&[(String, Expr)]>,
    ) {
        let Some(enum_sig) = self.enum_env.get(enum_name).cloned() else {
            self.add_error(format!("Enum '{}' not defined", enum_name));
            return;
        };
        let Some(variant_sig) = enum_sig.variants.get(variant_name).cloned() else {
            self.add_error(format!("Enum '{}::{}' not defined", enum_name, variant_name));
            return;
        };

        let actual_fields = fields.unwrap_or(&[]);
        if actual_fields.len() != variant_sig.fields.len() {
            self.add_error(format!(
                "enum variant '{}::{}' expects {} field(s), got {}",
                enum_name,
                variant_name,
                variant_sig.fields.len(),
                actual_fields.len()
            ));
            return;
        }

        let mut type_bindings = HashMap::new();
        for ((actual_name, actual_expr), (expected_name, expected_ty)) in
            actual_fields.iter().zip(variant_sig.fields.iter())
        {
            if actual_name != expected_name {
                self.add_error(format!(
                    "enum variant '{}::{}' expects field '{}', got '{}'",
                    enum_name, variant_name, expected_name, actual_name
                ));
                continue;
            }
            let Some(actual_ty) = self.infer_type(actual_expr) else {
                continue;
            };
            if let Some(expected_ty) = expected_ty {
                self.collect_type_bindings_with_generics(
                    &enum_sig.type_params,
                    expected_ty,
                    &actual_ty,
                    &mut type_bindings,
                );
                let resolved =
                    self.substitute_type_vars_with_generics(&enum_sig.type_params, expected_ty, &type_bindings);
                if !self.types_compatible(&resolved, &actual_ty) {
                    self.add_error(format!(
                        "enum variant '{}::{}' field '{}' expects {}, got {}",
                        enum_name,
                        variant_name,
                        actual_name,
                        resolved.display_name(),
                        actual_ty.display_name()
                    ));
                }
            }
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
        if args.len() != sig.params.len() {
            self.add_error(format!(
                "call '{}': expects {} arguments, got {}",
                fn_name,
                sig.params.len(),
                args.len()
            ));
            return;
        }

        let mut type_bindings = HashMap::new();
        for (arg, (_, param_type)) in args.iter().zip(sig.params.iter()) {
            let Some(param_ty) = param_type else {
                continue;
            };
            let Some(actual_ty) = self.infer_type(arg) else {
                continue;
            };
            self.collect_type_bindings_with_generics(&sig.type_params, param_ty, &actual_ty, &mut type_bindings);
        }

        for (index, (arg, (param_name, param_type))) in args.iter().zip(sig.params.iter()).enumerate() {
            let Some(expected) = param_type else {
                continue;
            };
            let Some(actual) = self.infer_type(arg) else {
                continue;
            };
            let resolved_expected =
                self.substitute_type_vars_with_generics(&sig.type_params, expected, &type_bindings);
            if !self.types_compatible(&resolved_expected, &actual) {
                self.add_error(format!(
                    "call '{}': argument {} ('{}') expects {}, got {}",
                    fn_name,
                    index + 1,
                    param_name,
                    resolved_expected.display_name(),
                    actual.display_name()
                ));
            }
        }

        for type_param in &sig.type_params {
            if !type_bindings.contains_key(type_param) {
                self.add_error(format!(
                    "call '{}': cannot infer generic type parameter '{}'",
                    fn_name, type_param
                ));
            }
        }
    }

    fn collect_type_bindings(
        &self,
        expected: &TypeAnnotation,
        actual: &TypeAnnotation,
        bindings: &mut HashMap<String, TypeAnnotation>,
    ) {
        let expected = self.normalize_type(expected);
        let actual = self.normalize_type(actual);
        match (expected, actual) {
            (TypeAnnotation::Named(name), actual) if self.is_generic_param(&name) => {
                bindings.entry(name).or_insert(actual);
            }
            (TypeAnnotation::ArrayOf(expected), TypeAnnotation::ArrayOf(actual)) => {
                self.collect_type_bindings(&expected, &actual, bindings);
            }
            (TypeAnnotation::MapOf(expected), TypeAnnotation::MapOf(actual)) => {
                self.collect_type_bindings(&expected, &actual, bindings);
            }
            (
                TypeAnnotation::Generic {
                    name: expected_name,
                    type_params: expected_params,
                },
                TypeAnnotation::Generic {
                    name: actual_name,
                    type_params: actual_params,
                },
            ) if expected_name == actual_name => {
                for (expected_param, actual_param) in expected_params.iter().zip(actual_params.iter()) {
                    self.collect_type_bindings(expected_param, actual_param, bindings);
                }
            }
            _ => {}
        }
    }

    fn collect_type_bindings_with_generics(
        &self,
        generic_params: &[String],
        expected: &TypeAnnotation,
        actual: &TypeAnnotation,
        bindings: &mut HashMap<String, TypeAnnotation>,
    ) {
        let expected = self.normalize_type(expected);
        let actual = self.normalize_type(actual);
        match (expected, actual) {
            (TypeAnnotation::Named(name), actual) if generic_params.iter().any(|param| param == &name) => {
                bindings.entry(name).or_insert(actual);
            }
            (TypeAnnotation::ArrayOf(expected), TypeAnnotation::ArrayOf(actual)) => {
                self.collect_type_bindings_with_generics(generic_params, &expected, &actual, bindings);
            }
            (TypeAnnotation::MapOf(expected), TypeAnnotation::MapOf(actual)) => {
                self.collect_type_bindings_with_generics(generic_params, &expected, &actual, bindings);
            }
            (
                TypeAnnotation::Generic {
                    name: expected_name,
                    type_params: expected_params,
                },
                TypeAnnotation::Generic {
                    name: actual_name,
                    type_params: actual_params,
                },
            ) if expected_name == actual_name => {
                for (expected_param, actual_param) in expected_params.iter().zip(actual_params.iter()) {
                    self.collect_type_bindings_with_generics(
                        generic_params,
                        expected_param,
                        actual_param,
                        bindings,
                    );
                }
            }
            _ => {}
        }
    }

    fn substitute_type_vars(
        &self,
        ty: &TypeAnnotation,
        bindings: &HashMap<String, TypeAnnotation>,
    ) -> TypeAnnotation {
        match self.normalize_type(ty) {
            TypeAnnotation::Named(name) if self.is_generic_param(&name) => {
                bindings.get(&name).cloned().unwrap_or(TypeAnnotation::Named(name))
            }
            TypeAnnotation::ArrayOf(inner) => {
                TypeAnnotation::ArrayOf(Box::new(self.substitute_type_vars(&inner, bindings)))
            }
            TypeAnnotation::MapOf(inner) => {
                TypeAnnotation::MapOf(Box::new(self.substitute_type_vars(&inner, bindings)))
            }
            TypeAnnotation::Generic { name, type_params } => TypeAnnotation::Generic {
                name,
                type_params: type_params
                    .iter()
                    .map(|param| self.substitute_type_vars(param, bindings))
                    .collect(),
            },
            other => other,
        }
    }

    fn substitute_type_vars_with_generics(
        &self,
        generic_params: &[String],
        ty: &TypeAnnotation,
        bindings: &HashMap<String, TypeAnnotation>,
    ) -> TypeAnnotation {
        match self.normalize_type(ty) {
            TypeAnnotation::Named(name) if generic_params.iter().any(|param| param == &name) => {
                bindings.get(&name).cloned().unwrap_or(TypeAnnotation::Named(name))
            }
            TypeAnnotation::ArrayOf(inner) => TypeAnnotation::ArrayOf(Box::new(
                self.substitute_type_vars_with_generics(generic_params, &inner, bindings),
            )),
            TypeAnnotation::MapOf(inner) => TypeAnnotation::MapOf(Box::new(
                self.substitute_type_vars_with_generics(generic_params, &inner, bindings),
            )),
            TypeAnnotation::Generic { name, type_params } => TypeAnnotation::Generic {
                name,
                type_params: type_params
                    .iter()
                    .map(|param| self.substitute_type_vars_with_generics(generic_params, param, bindings))
                    .collect(),
            },
            other => other,
        }
    }

    fn types_compatible(&self, expected: &TypeAnnotation, actual: &TypeAnnotation) -> bool {
        let expected = self.normalize_type(expected);
        let actual = self.normalize_type(actual);
        match (&expected, &actual) {
            (TypeAnnotation::Any, _) | (_, TypeAnnotation::Any) => true,
            (TypeAnnotation::Named(name), _) if self.is_generic_param(name) => true,
            (_, TypeAnnotation::Named(name)) if self.is_generic_param(name) => true,
            (TypeAnnotation::Int, TypeAnnotation::Int) => true,
            (TypeAnnotation::Str, TypeAnnotation::Str) => true,
            (TypeAnnotation::Bool, TypeAnnotation::Bool) => true,
            (TypeAnnotation::ArrayOf(expected), TypeAnnotation::ArrayOf(actual)) => {
                self.types_compatible(expected, actual)
            }
            (TypeAnnotation::MapOf(expected), TypeAnnotation::MapOf(actual)) => {
                self.types_compatible(expected, actual)
            }
            (
                TypeAnnotation::Generic {
                    name: expected_name,
                    type_params: expected_params,
                },
                TypeAnnotation::Generic {
                    name: actual_name,
                    type_params: actual_params,
                },
            ) => {
                expected_name == actual_name
                    && expected_params.len() == actual_params.len()
                    && expected_params
                        .iter()
                        .zip(actual_params.iter())
                        .all(|(expected, actual)| self.types_compatible(expected, actual))
            }
            (TypeAnnotation::Named(expected_name), TypeAnnotation::Named(actual_name)) => {
                expected_name == actual_name
                    || (self.interface_env.contains_key(expected_name)
                        && self.struct_implements_interface(actual_name, expected_name))
            }
            _ => false,
        }
    }

    fn enum_type_from_sig(&self, name: &str, sig: &EnumSig) -> TypeAnnotation {
        if sig.type_params.is_empty() {
            TypeAnnotation::Named(name.into())
        } else {
            TypeAnnotation::Generic {
                name: name.into(),
                type_params: sig
                    .type_params
                    .iter()
                    .map(|param| TypeAnnotation::Named(param.clone()))
                    .collect(),
            }
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
    fn normalize(ty: &TypeAnnotation) -> TypeAnnotation {
        match ty {
            TypeAnnotation::Generic { name, type_params } if name == "Array" && type_params.len() == 1 => {
                TypeAnnotation::ArrayOf(Box::new(normalize(&type_params[0])))
            }
            TypeAnnotation::Generic { name, type_params } if name == "Map" && type_params.len() == 1 => {
                TypeAnnotation::MapOf(Box::new(normalize(&type_params[0])))
            }
            TypeAnnotation::ArrayOf(inner) => TypeAnnotation::ArrayOf(Box::new(normalize(inner))),
            TypeAnnotation::MapOf(inner) => TypeAnnotation::MapOf(Box::new(normalize(inner))),
            TypeAnnotation::Generic { name, type_params } => TypeAnnotation::Generic {
                name: name.clone(),
                type_params: type_params.iter().map(normalize).collect(),
            },
            other => other.clone(),
        }
    }

    match (normalize(expected), normalize(actual)) {
        (TypeAnnotation::Any, _) | (_, TypeAnnotation::Any) => true,
        (TypeAnnotation::Int, TypeAnnotation::Int) => true,
        (TypeAnnotation::Str, TypeAnnotation::Str) => true,
        (TypeAnnotation::Bool, TypeAnnotation::Bool) => true,
        (TypeAnnotation::ArrayOf(expected), TypeAnnotation::ArrayOf(actual)) => {
            types_compatible(&expected, &actual)
        }
        (TypeAnnotation::MapOf(expected), TypeAnnotation::MapOf(actual)) => {
            types_compatible(&expected, &actual)
        }
        (
            TypeAnnotation::Generic {
                name: expected_name,
                type_params: expected_params,
            },
            TypeAnnotation::Generic {
                name: actual_name,
                type_params: actual_params,
            },
        ) => {
            expected_name == actual_name
                && expected_params.len() == actual_params.len()
                && expected_params
                    .iter()
                    .zip(actual_params.iter())
                    .all(|(expected, actual)| types_compatible(expected, actual))
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

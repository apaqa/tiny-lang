//! tiny-lang formatter。

use crate::ast::{
    BinaryOperator, EnumVariant, Expr, InterfaceMethod, Pattern, Program, Statement, TypeAnnotation,
    UnaryOperator,
};

/// 中文註解：對 AST 進行 pretty-print，輸出穩定格式的 tiny-lang 原始碼。
pub fn format_program(program: &Program) -> String {
    Formatter::new().format_program(program)
}

struct Formatter {
    output: String,
    indent_level: usize,
    previous_was_fn_like: bool,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            previous_was_fn_like: false,
        }
    }

    fn format_program(mut self, program: &Program) -> String {
        for (index, statement) in program.iter().enumerate() {
            let is_fn_like = matches!(
                statement,
                Statement::FnDecl { .. } | Statement::MethodDecl { .. } | Statement::ImplInterface { .. }
            );
            if index > 0 {
                self.output.push('\n');
                if is_fn_like || self.previous_was_fn_like {
                    self.output.push('\n');
                }
            }
            self.write_statement(statement);
            self.previous_was_fn_like = is_fn_like;
        }
        self.output
    }

    fn write_statement(&mut self, statement: &Statement) {
        self.write_indent();
        match statement {
            Statement::Import { path } => {
                self.output.push_str("import ");
                self.output.push_str(&self.format_string_literal(path));
                self.output.push(';');
            }
            Statement::StructDecl { name, fields } => {
                self.output.push_str(&format!("struct {name} {{"));
                if fields.is_empty() {
                    self.output.push_str(" }");
                } else {
                    self.output.push('\n');
                    self.indent_level += 1;
                    for (index, (field, annotation)) in fields.iter().enumerate() {
                        self.write_indent();
                        self.output.push_str(field);
                        if let Some(annotation) = annotation {
                            self.output.push_str(": ");
                            self.output.push_str(&self.format_type(annotation));
                        }
                        if index + 1 < fields.len() {
                            self.output.push(',');
                        }
                        self.output.push('\n');
                    }
                    self.indent_level -= 1;
                    self.write_indent();
                    self.output.push('}');
                }
            }
            Statement::InterfaceDecl { name, methods } => {
                self.output.push_str(&format!("interface {name} {{"));
                if methods.is_empty() {
                    self.output.push_str(" }");
                } else {
                    self.output.push('\n');
                    self.indent_level += 1;
                    for method in methods {
                        self.write_indent();
                        self.write_interface_method(method);
                        self.output.push('\n');
                    }
                    self.indent_level -= 1;
                    self.write_indent();
                    self.output.push('}');
                }
            }
            Statement::ImplInterface {
                interface_name,
                struct_name,
                methods,
            } => {
                self.output
                    .push_str(&format!("impl {interface_name} for {struct_name} "));
                self.output.push_str("{\n");
                self.indent_level += 1;
                for method in methods {
                    self.write_statement(method);
                    self.output.push('\n');
                }
                self.indent_level -= 1;
                self.write_indent();
                self.output.push('}');
            }
            Statement::EnumDecl { name, variants } => {
                self.output.push_str(&format!("enum {name} {{"));
                if variants.is_empty() {
                    self.output.push_str(" }");
                } else {
                    self.output.push('\n');
                    self.indent_level += 1;
                    for (index, variant) in variants.iter().enumerate() {
                        self.write_indent();
                        self.output.push_str(&self.format_enum_variant_decl(variant));
                        if index + 1 < variants.len() {
                            self.output.push(',');
                        }
                        self.output.push('\n');
                    }
                    self.indent_level -= 1;
                    self.write_indent();
                    self.output.push('}');
                }
            }
            Statement::LetDecl {
                name,
                type_annotation,
                value,
            } => {
                self.output.push_str("let ");
                self.output.push_str(name);
                if let Some(annotation) = type_annotation {
                    self.output.push_str(": ");
                    self.output.push_str(&self.format_type(annotation));
                }
                self.output.push_str(" = ");
                self.output.push_str(&self.format_expr(value));
                self.output.push(';');
            }
            Statement::Assignment { name, value } => {
                self.output.push_str(name);
                self.output.push_str(" = ");
                self.output.push_str(&self.format_expr(value));
                self.output.push(';');
            }
            Statement::IndexAssignment { target, index, value } => {
                self.output.push_str(&self.format_expr(target));
                self.output.push('[');
                self.output.push_str(&self.format_expr(index));
                self.output.push_str("] = ");
                self.output.push_str(&self.format_expr(value));
                self.output.push(';');
            }
            Statement::FieldAssignment { object, field, value } => {
                self.output.push_str(&self.format_expr(object));
                self.output.push('.');
                self.output.push_str(field);
                self.output.push_str(" = ");
                self.output.push_str(&self.format_expr(value));
                self.output.push(';');
            }
            Statement::FnDecl {
                name,
                params,
                return_type,
                body,
            } => {
                self.output.push_str("fn ");
                self.output.push_str(name);
                self.write_function_signature(params, return_type);
                self.output.push(' ');
                self.write_block(body);
            }
            Statement::MethodDecl {
                struct_name,
                method_name,
                params,
                return_type,
                body,
            } => {
                self.output.push_str("fn ");
                self.output.push_str(struct_name);
                self.output.push('.');
                self.output.push_str(method_name);
                self.write_function_signature(params, return_type);
                self.output.push(' ');
                self.write_block(body);
            }
            Statement::Return(expr) => {
                self.output.push_str("return ");
                self.output.push_str(&self.format_expr(expr));
                self.output.push(';');
            }
            Statement::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                self.output.push_str("if ");
                self.output.push_str(&self.format_expr(condition));
                self.output.push(' ');
                self.write_block(then_body);
                if let Some(else_body) = else_body {
                    self.output.push_str(" else ");
                    self.write_block(else_body);
                }
            }
            Statement::While { condition, body } => {
                self.output.push_str("while ");
                self.output.push_str(&self.format_expr(condition));
                self.output.push(' ');
                self.write_block(body);
            }
            Statement::ForLoop {
                variable,
                iterable,
                body,
            } => {
                self.output.push_str("for ");
                self.output.push_str(variable);
                self.output.push_str(" in ");
                self.output.push_str(&self.format_expr(iterable));
                self.output.push(' ');
                self.write_block(body);
            }
            Statement::Break => self.output.push_str("break;"),
            Statement::Continue => self.output.push_str("continue;"),
            Statement::TryCatch {
                try_body,
                catch_var,
                catch_body,
            } => {
                self.output.push_str("try ");
                self.write_block(try_body);
                self.output.push_str(" catch ");
                self.output.push_str(catch_var);
                self.output.push(' ');
                self.write_block(catch_body);
            }
            Statement::Match { expr, arms } => {
                self.output.push_str("match ");
                self.output.push_str(&self.format_expr(expr));
                self.output.push_str(" {\n");
                self.indent_level += 1;
                for arm in arms {
                    self.write_indent();
                    self.output.push_str(&self.format_pattern(&arm.pattern));
                    self.output.push_str(" => ");
                    self.write_inline_block(&arm.body);
                    self.output.push('\n');
                }
                self.indent_level -= 1;
                self.write_indent();
                self.output.push('}');
            }
            Statement::Print(expr) => {
                self.output.push_str("print(");
                self.output.push_str(&self.format_expr(expr));
                self.output.push_str(");");
            }
            Statement::ExprStatement(expr) => {
                self.output.push_str(&self.format_expr(expr));
                self.output.push(';');
            }
        }
    }

    fn write_function_signature(
        &mut self,
        params: &[(String, Option<TypeAnnotation>)],
        return_type: &Option<TypeAnnotation>,
    ) {
        self.output.push('(');
        for (index, (name, annotation)) in params.iter().enumerate() {
            if index > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(name);
            if let Some(annotation) = annotation {
                self.output.push_str(": ");
                self.output.push_str(&self.format_type(annotation));
            }
        }
        self.output.push(')');
        if let Some(return_type) = return_type {
            self.output.push_str(" -> ");
            self.output.push_str(&self.format_type(return_type));
        }
    }

    fn write_interface_method(&mut self, method: &InterfaceMethod) {
        self.output.push_str("fn ");
        self.output.push_str(&method.name);
        self.write_function_signature(&method.params, &method.return_type);
        self.output.push(';');
    }

    fn write_block(&mut self, body: &[Statement]) {
        self.output.push_str("{\n");
        self.indent_level += 1;
        for statement in body {
            self.write_statement(statement);
            self.output.push('\n');
        }
        self.indent_level -= 1;
        self.write_indent();
        self.output.push('}');
    }

    fn write_inline_block(&mut self, body: &[Statement]) {
        self.output.push_str("{\n");
        self.indent_level += 1;
        for statement in body {
            self.write_statement(statement);
            self.output.push('\n');
        }
        self.indent_level -= 1;
        self.write_indent();
        self.output.push('}');
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str("    ");
        }
    }

    fn format_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::IntLit(value) => value.to_string(),
            Expr::StringLit(value) => self.format_string_literal(value),
            Expr::BoolLit(value) => value.to_string(),
            Expr::NullLit => "null".into(),
            Expr::Ident(name) => name.clone(),
            Expr::StructInit { name, fields } => {
                let items = fields
                    .iter()
                    .map(|(field, value)| format!("{field}: {}", self.format_expr(value)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name} {{ {items} }}")
            }
            Expr::EnumVariant {
                enum_name,
                variant,
                fields,
            } => match fields {
                Some(fields) if !fields.is_empty() => {
                    let items = fields
                        .iter()
                        .map(|(field, value)| format!("{field}: {}", self.format_expr(value)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{enum_name}::{variant} {{ {items} }}")
                }
                _ => format!("{enum_name}::{variant}"),
            },
            Expr::ArrayLit(items) => {
                let items = items
                    .iter()
                    .map(|item| self.format_expr(item))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{items}]")
            }
            Expr::MapLit(items) => {
                let items = items
                    .iter()
                    .map(|(key, value)| format!("{}: {}", self.format_expr(key), self.format_expr(value)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{items}}}")
            }
            Expr::IndexAccess { target, index } => {
                format!("{}[{}]", self.format_expr(target), self.format_expr(index))
            }
            Expr::FieldAccess { object, field } => format!("{}.{}", self.format_expr(object), field),
            Expr::BinaryOp { left, op, right } => format!(
                "{} {} {}",
                self.format_wrapped_expr(left),
                self.format_binary_operator(*op),
                self.format_wrapped_expr(right)
            ),
            Expr::UnaryOp { op, operand } => {
                format!("{}{}", self.format_unary_operator(*op), self.format_wrapped_expr(operand))
            }
            Expr::FnCall { callee, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.format_expr(arg))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({args})", self.format_expr(callee))
            }
            Expr::Lambda { params, body } => {
                let params = params.join(", ");
                let mut nested = Formatter {
                    output: String::new(),
                    indent_level: self.indent_level,
                    previous_was_fn_like: false,
                };
                nested.output.push('|');
                nested.output.push_str(&params);
                nested.output.push('|');
                nested.output.push(' ');
                nested.write_block(body);
                nested.output
            }
        }
    }

    fn format_wrapped_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::BinaryOp { .. } => format!("({})", self.format_expr(expr)),
            _ => self.format_expr(expr),
        }
    }

    fn format_pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::IntLit(value) => value.to_string(),
            Pattern::StringLit(value) => self.format_string_literal(value),
            Pattern::BoolLit(value) => value.to_string(),
            Pattern::Ident(name) => name.clone(),
            Pattern::EnumVariant {
                enum_name,
                variant,
                bindings,
            } => match bindings {
                Some(bindings) if !bindings.is_empty() => {
                    format!("{enum_name}::{variant}({})", bindings.join(", "))
                }
                _ => format!("{enum_name}::{variant}"),
            },
            Pattern::Wildcard => "_".into(),
        }
    }

    fn format_type(&self, annotation: &TypeAnnotation) -> String {
        match annotation {
            TypeAnnotation::Int => "int".into(),
            TypeAnnotation::Str => "str".into(),
            TypeAnnotation::Bool => "bool".into(),
            TypeAnnotation::ArrayOf(inner) => format!("[{}]", self.format_type(inner)),
            TypeAnnotation::MapOf(inner) => format!("{{{}}}", self.format_type(inner)),
            TypeAnnotation::Named(name) => name.clone(),
            TypeAnnotation::Any => "any".into(),
        }
    }

    fn format_enum_variant_decl(&self, variant: &EnumVariant) -> String {
        match &variant.fields {
            Some(fields) if !fields.is_empty() => {
                let fields = fields
                    .iter()
                    .map(|(name, annotation)| match annotation {
                        Some(annotation) => format!("{name}: {}", self.format_type(annotation)),
                        None => name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {{ {} }}", variant.name, fields)
            }
            _ => variant.name.clone(),
        }
    }

    fn format_binary_operator(&self, op: BinaryOperator) -> &'static str {
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

    fn format_unary_operator(&self, op: UnaryOperator) -> &'static str {
        match op {
            UnaryOperator::Neg => "-",
            UnaryOperator::Not => "!",
        }
    }

    fn format_string_literal(&self, value: &str) -> String {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        format!("\"{escaped}\"")
    }
}

//! 靜態型別檢查器。
//!
//! 在 parse 之後、執行之前走訪整個 AST，以「盡可能收集所有錯誤」為目標，
//! 不在第一個錯誤時停止。
//!
//! 能檢查的項目：
//! - `let x: int = "hello"` 型別不符
//! - 函式呼叫引數型別不符宣告的參數型別
//! - `return` 表達式型別不符函式宣告的回傳型別
//! - 二元運算元型別不相容（`1 + "hello"` 等）
//! - 一元運算元型別不符（`-true`、`!0` 等）
//! - 變數賦值型別不符原始宣告型別
//!
//! 無法靜態推斷型別時（例如未型別化變數、函式回傳值未標記）一律跳過，
//! 不產生誤報。

use std::collections::HashMap;

use crate::ast::{BinaryOperator, Expr, MatchArm, Program, Statement, TypeAnnotation, UnaryOperator};
use crate::error::TinyLangError;

// ─── 函式簽名 ────────────────────────────────────────────────────────────────

/// 記錄函式的參數型別列表與回傳型別，供呼叫點檢查使用。
#[derive(Debug, Clone)]
struct FuncSig {
    /// (參數名稱, 參數型別)
    params: Vec<(String, Option<TypeAnnotation>)>,
    /// 回傳型別
    return_type: Option<TypeAnnotation>,
}

// ─── TypeChecker ─────────────────────────────────────────────────────────────

/// 靜態型別檢查器主體。
///
/// 使用方式：
/// ```ignore
/// let mut checker = TypeChecker::new();
/// checker.check_program(&program);
/// if !checker.errors.is_empty() {
///     // 回報所有錯誤
/// }
/// ```
pub struct TypeChecker {
    /// 變數型別環境（巢狀 scope）
    type_env: Vec<HashMap<String, TypeAnnotation>>,
    /// 已知函式簽名
    func_env: HashMap<String, FuncSig>,
    /// 目前所在函式的宣告回傳型別（用於驗證 return）
    current_return_type: Option<TypeAnnotation>,
    /// 蒐集到的所有型別錯誤
    pub errors: Vec<TinyLangError>,
}

impl TypeChecker {
    /// 建立新的型別檢查器。
    pub fn new() -> Self {
        Self {
            type_env: vec![HashMap::new()],
            func_env: HashMap::new(),
            current_return_type: None,
            errors: Vec::new(),
        }
    }

    /// 對整個程式執行型別檢查。
    ///
    /// 採用兩趟掃描：
    /// 1. 先收集所有頂層函式簽名，讓互相呼叫的函式能互相查到型別。
    /// 2. 再走訪所有陳述式進行完整型別檢查。
    pub fn check_program(&mut self, program: &Program) {
        // 第一趟：收集頂層函式簽名
        for stmt in program {
            if let Statement::FnDecl { name, params, return_type, .. } = stmt {
                self.func_env.insert(
                    name.clone(),
                    FuncSig {
                        params: params.clone(),
                        return_type: return_type.clone(),
                    },
                );
            }
        }
        // 第二趟：完整型別檢查
        for stmt in program {
            self.check_statement(stmt);
        }
    }

    // ── scope 管理 ──────────────────────────────────────────────────────────

    fn push_scope(&mut self) {
        self.type_env.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.type_env.pop();
    }

    /// 在目前 scope 宣告一個有型別的變數。
    fn declare_var(&mut self, name: &str, ty: TypeAnnotation) {
        if let Some(scope) = self.type_env.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    /// 從最近的 scope 開始向外查找變數的型別。
    fn lookup_var(&self, name: &str) -> Option<&TypeAnnotation> {
        for scope in self.type_env.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// 記錄一個型別錯誤。
    fn add_error(&mut self, message: String) {
        self.errors.push(TinyLangError::type_check(message));
    }

    // ── 型別推斷 ────────────────────────────────────────────────────────────

    /// 嘗試靜態推斷表達式的型別。
    ///
    /// 若無法確定（例如未型別化的識別子、未知函式的回傳值）則回傳 `None`。
    /// 不會修改任何狀態，因此可在 `&self` 下呼叫。
    fn infer_type(&self, expr: &Expr) -> Option<TypeAnnotation> {
        match expr {
            // 字面值型別直接確定
            Expr::IntLit(_) => Some(TypeAnnotation::Int),
            Expr::StringLit(_) => Some(TypeAnnotation::Str),
            Expr::BoolLit(_) => Some(TypeAnnotation::Bool),

            // 識別子：查型別環境
            Expr::Ident(name) => self.lookup_var(name).cloned(),

            // 二元運算：只在兩個運算元型別都已知時才推斷結果
            Expr::BinaryOp { op, left, right } => {
                let lt = self.infer_type(left);
                let rt = self.infer_type(right);
                self.infer_binop_type(*op, lt.as_ref(), rt.as_ref())
            }

            // 一元運算
            Expr::UnaryOp { op, operand } => {
                let ot = self.infer_type(operand);
                match op {
                    UnaryOperator::Neg => {
                        if matches!(ot, Some(TypeAnnotation::Int)) {
                            Some(TypeAnnotation::Int)
                        } else {
                            None
                        }
                    }
                    UnaryOperator::Not => Some(TypeAnnotation::Bool),
                }
            }

            // 函式呼叫：查函式簽名取得回傳型別
            Expr::FnCall { callee, .. } => {
                if let Expr::Ident(name) = callee.as_ref() {
                    self.func_env.get(name).and_then(|sig| sig.return_type.clone())
                } else {
                    None
                }
            }

            // 陣列字面值：以第一個元素的型別作為元素型別
            Expr::ArrayLit(items) => {
                let first_ty = items.first().and_then(|e| self.infer_type(e))?;
                Some(TypeAnnotation::ArrayOf(Box::new(first_ty)))
            }

            // 其他情況無法靜態推斷
            _ => None,
        }
    }

    /// 推斷二元運算的結果型別。
    fn infer_binop_type(
        &self,
        op: BinaryOperator,
        lt: Option<&TypeAnnotation>,
        rt: Option<&TypeAnnotation>,
    ) -> Option<TypeAnnotation> {
        match op {
            // 算術：int + int → int；str + str → str（字串拼接）
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
            // 比較/邏輯：結果一律為 bool
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

    // ── 陳述式檢查 ──────────────────────────────────────────────────────────

    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            // let 宣告：若有型別標記則驗證初始值
            Statement::LetDecl { name, type_annotation, value } => {
                // 先推斷值的型別（在宣告變數前，避免自我參照問題）
                let val_ty = self.infer_type(value);
                if let Some(ann) = type_annotation {
                    self.check_compatible(
                        ann,
                        val_ty.clone(),
                        &format!("let {name}: 宣告型別為 {}, 但初始值型別為", ann.display_name()),
                    );
                    self.declare_var(name, ann.clone());
                } else if let Some(ty) = val_ty.clone() {
                    // 無型別標記：從初始值推斷並記錄，供後續賦值檢查使用
                    self.declare_var(name, ty);
                }
                self.check_expr(value);
            }

            // 賦值：型別必須與原始宣告相容
            Statement::Assignment { name, value } => {
                let val_ty = self.infer_type(value);
                if let Some(declared) = self.lookup_var(name).cloned() {
                    self.check_compatible(
                        &declared,
                        val_ty,
                        &format!("賦值 {name}: 宣告型別為 {}, 但賦值型別為", declared.display_name()),
                    );
                }
                self.check_expr(value);
            }

            // 函式宣告：簽名已在第一趟收集，這裡檢查函式主體
            Statement::FnDecl { name, params, return_type, body } => {
                // 確保簽名已登記（處理巢狀函式的情況）
                self.func_env.insert(
                    name.clone(),
                    FuncSig { params: params.clone(), return_type: return_type.clone() },
                );
                self.push_scope();
                for (param_name, param_type) in params {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, ty.clone());
                    }
                }
                let outer_ret = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                for s in body {
                    self.check_statement(s);
                }
                self.current_return_type = outer_ret;
                self.pop_scope();
            }

            // 方法宣告
            Statement::MethodDecl { params, body, return_type, .. } => {
                self.push_scope();
                for (param_name, param_type) in params {
                    if let Some(ty) = param_type {
                        self.declare_var(param_name, ty.clone());
                    }
                }
                let outer_ret = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                for s in body {
                    self.check_statement(s);
                }
                self.current_return_type = outer_ret;
                self.pop_scope();
            }

            // return：驗證回傳值型別是否符合函式宣告
            Statement::Return(expr) => {
                if let Some(expected) = self.current_return_type.clone() {
                    let actual = self.infer_type(expr);
                    self.check_compatible(
                        &expected,
                        actual,
                        &format!(
                            "return: 函式宣告回傳型別為 {}, 但實際回傳型別為",
                            expected.display_name()
                        ),
                    );
                }
                self.check_expr(expr);
            }

            Statement::IfElse { condition, then_body, else_body } => {
                self.check_expr(condition);
                self.push_scope();
                for s in then_body {
                    self.check_statement(s);
                }
                self.pop_scope();
                if let Some(else_stmts) = else_body {
                    self.push_scope();
                    for s in else_stmts {
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

            Statement::ForLoop { variable, iterable, body } => {
                self.check_expr(iterable);
                self.push_scope();
                // 若 iterable 為已知陣列型別，將迭代變數登記為元素型別
                if let Some(TypeAnnotation::ArrayOf(elem_ty)) = self.infer_type(iterable) {
                    self.declare_var(variable, *elem_ty);
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

            Statement::TryCatch { try_body, catch_var, catch_body } => {
                self.push_scope();
                for s in try_body {
                    self.check_statement(s);
                }
                self.pop_scope();
                self.push_scope();
                // catch 變數為字串型別（錯誤訊息）
                self.declare_var(catch_var, TypeAnnotation::Str);
                for s in catch_body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }

            Statement::Print(expr) => self.check_expr(expr),
            Statement::ExprStatement(expr) => self.check_expr(expr),

            Statement::IndexAssignment { target, index, value } => {
                self.check_expr(target);
                self.check_expr(index);
                self.check_expr(value);
            }

            Statement::FieldAssignment { object, value, .. } => {
                self.check_expr(object);
                self.check_expr(value);
            }

            // 以下陳述式無需型別檢查
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

    // ── 表達式檢查 ──────────────────────────────────────────────────────────

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::BinaryOp { op, left, right } => {
                self.check_expr(left);
                self.check_expr(right);
                // 取得兩側型別後再做相容性驗證
                let lt = self.infer_type(left);
                let rt = self.infer_type(right);
                self.check_binop(*op, lt, rt);
            }

            Expr::UnaryOp { op, operand } => {
                self.check_expr(operand);
                let ot = self.infer_type(operand);
                self.check_unaryop(*op, ot);
            }

            Expr::FnCall { callee, args } => {
                self.check_expr(callee);
                for arg in args {
                    self.check_expr(arg);
                }
                // 若可查到函式簽名，逐一驗證引數型別
                if let Expr::Ident(name) = callee.as_ref() {
                    let sig = self.func_env.get(name).cloned();
                    if let Some(sig) = sig {
                        self.check_call_args(name, args, &sig);
                    }
                }
            }

            Expr::ArrayLit(items) => {
                for item in items {
                    self.check_expr(item);
                }
            }

            Expr::MapLit(pairs) => {
                for (k, v) in pairs {
                    self.check_expr(k);
                    self.check_expr(v);
                }
            }

            Expr::IndexAccess { target, index } => {
                self.check_expr(target);
                self.check_expr(index);
            }

            Expr::FieldAccess { object, .. } => {
                self.check_expr(object);
            }

            Expr::StructInit { fields, .. } => {
                for (_, v) in fields {
                    self.check_expr(v);
                }
            }

            Expr::EnumVariant { fields, .. } => {
                if let Some(fields) = fields {
                    for (_, v) in fields {
                        self.check_expr(v);
                    }
                }
            }

            // lambda：在新 scope 下走訪主體，不帶參數型別（lambda 參數無型別標記）
            Expr::Lambda { body, .. } => {
                self.push_scope();
                for s in body {
                    self.check_statement(s);
                }
                self.pop_scope();
            }

            // 字面值、識別子無需額外檢查
            Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) | Expr::Ident(_) => {}
        }
    }

    // ── 具體檢查邏輯 ────────────────────────────────────────────────────────

    /// 檢查 `actual` 是否與 `expected` 型別相容。
    ///
    /// `context_prefix` 為錯誤訊息的前綴，函式會在後面附上實際型別名稱。
    fn check_compatible(
        &mut self,
        expected: &TypeAnnotation,
        actual: Option<TypeAnnotation>,
        context_prefix: &str,
    ) {
        if let Some(actual_ty) = actual {
            if !types_compatible(expected, &actual_ty) {
                self.add_error(format!("{} {}", context_prefix, actual_ty.display_name()));
            }
        }
        // actual 為 None 表示無法靜態推斷，跳過以避免誤報
    }

    /// 驗證二元運算元型別是否相容。
    fn check_binop(
        &mut self,
        op: BinaryOperator,
        lt: Option<TypeAnnotation>,
        rt: Option<TypeAnnotation>,
    ) {
        // 只在兩側型別都已知時才做檢查
        let (Some(lt), Some(rt)) = (&lt, &rt) else { return };

        match op {
            BinaryOperator::Add => {
                let ok = matches!(
                    (lt, rt),
                    (TypeAnnotation::Int, TypeAnnotation::Int)
                        | (TypeAnnotation::Str, TypeAnnotation::Str)
                );
                if !ok {
                    self.add_error(format!(
                        "二元 '+': 無法對 {} 和 {} 執行加法",
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::Sub | BinaryOperator::Mul | BinaryOperator::Div | BinaryOperator::Mod => {
                if !matches!((lt, rt), (TypeAnnotation::Int, TypeAnnotation::Int)) {
                    self.add_error(format!(
                        "二元 '{}': 需要 int 運算元，但得到 {} 和 {}",
                        binop_symbol(op),
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::Lt | BinaryOperator::Gt | BinaryOperator::Le | BinaryOperator::Ge => {
                // 只允許 int 或 str 之間的大小比較
                let ok = matches!(
                    (lt, rt),
                    (TypeAnnotation::Int, TypeAnnotation::Int)
                        | (TypeAnnotation::Str, TypeAnnotation::Str)
                );
                if !ok {
                    self.add_error(format!(
                        "比較運算 '{}': 型別不相容（{} 和 {}）",
                        binop_symbol(op),
                        lt.display_name(),
                        rt.display_name()
                    ));
                }
            }
            BinaryOperator::And | BinaryOperator::Or => {
                // 邏輯運算需要 bool 運算元
                if !matches!(lt, TypeAnnotation::Bool) {
                    self.add_error(format!(
                        "邏輯運算 '{}': 左運算元需要 bool，但得到 {}",
                        binop_symbol(op),
                        lt.display_name()
                    ));
                }
                if !matches!(rt, TypeAnnotation::Bool) {
                    self.add_error(format!(
                        "邏輯運算 '{}': 右運算元需要 bool，但得到 {}",
                        binop_symbol(op),
                        rt.display_name()
                    ));
                }
            }
            // Eq/Ne：任意同型別均可比較，不做額外限制
            BinaryOperator::Eq | BinaryOperator::Ne => {}
        }
    }

    /// 驗證一元運算元型別是否合法。
    fn check_unaryop(&mut self, op: UnaryOperator, operand_ty: Option<TypeAnnotation>) {
        let Some(ty) = operand_ty else { return };
        match op {
            UnaryOperator::Neg => {
                if !matches!(ty, TypeAnnotation::Int) {
                    self.add_error(format!(
                        "一元 '-': 需要 int，但得到 {}",
                        ty.display_name()
                    ));
                }
            }
            UnaryOperator::Not => {
                if !matches!(ty, TypeAnnotation::Bool) {
                    self.add_error(format!(
                        "一元 '!': 需要 bool，但得到 {}",
                        ty.display_name()
                    ));
                }
            }
        }
    }

    /// 驗證函式呼叫的引數型別是否符合簽名。
    fn check_call_args(&mut self, fn_name: &str, args: &[Expr], sig: &FuncSig) {
        for (i, (arg, (param_name, param_type))) in
            args.iter().zip(sig.params.iter()).enumerate()
        {
            let Some(expected) = param_type else { continue };
            let Some(actual) = self.infer_type(arg) else { continue };
            if !types_compatible(expected, &actual) {
                self.add_error(format!(
                    "函式 '{}': 第 {} 個引數 ('{}') 型別不符，預期 {}，得到 {}",
                    fn_name,
                    i + 1,
                    param_name,
                    expected.display_name(),
                    actual.display_name()
                ));
            }
        }
    }
}

// ─── 輔助函式 ─────────────────────────────────────────────────────────────────

/// 判斷兩個型別是否相容（支援 Any 萬用型別）。
pub fn types_compatible(expected: &TypeAnnotation, actual: &TypeAnnotation) -> bool {
    match (expected, actual) {
        // Any 可與任意型別相容
        (TypeAnnotation::Any, _) | (_, TypeAnnotation::Any) => true,
        (TypeAnnotation::Int, TypeAnnotation::Int) => true,
        (TypeAnnotation::Str, TypeAnnotation::Str) => true,
        (TypeAnnotation::Bool, TypeAnnotation::Bool) => true,
        (TypeAnnotation::ArrayOf(e), TypeAnnotation::ArrayOf(a)) => types_compatible(e, a),
        (TypeAnnotation::MapOf(e), TypeAnnotation::MapOf(a)) => types_compatible(e, a),
        (TypeAnnotation::Named(e), TypeAnnotation::Named(a)) => e == a,
        _ => false,
    }
}

/// 回傳二元運算子的符號字串（用於錯誤訊息）。
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

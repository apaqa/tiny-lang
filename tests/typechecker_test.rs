//! 靜態型別檢查器測試。
//!
//! 測試項目：
//! - let 宣告型別不符在編譯期被發現
//! - 函式參數型別不符
//! - 函式回傳型別不符
//! - 二元運算型別不相容
//! - 一次收集多個錯誤
//! - 無型別標記的程式碼不受影響（不產生誤報）
//! - 正確的型別標記程式碼通過檢查

use tiny_lang::ast::Program;
use tiny_lang::parse_source;
use tiny_lang::typechecker::TypeChecker;

/// 執行型別檢查並回傳錯誤列表。
fn type_errors(source: &str) -> Vec<String> {
    let program: Program = parse_source(source).expect("parse 應成功");
    let mut checker = TypeChecker::new();
    checker.check_program(&program);
    checker.errors.iter().map(|e| e.message.clone()).collect()
}

/// 斷言程式碼通過型別檢查（無錯誤）。
fn assert_passes(source: &str) {
    let errors = type_errors(source);
    assert!(
        errors.is_empty(),
        "預期型別檢查通過，但得到錯誤：{:?}",
        errors
    );
}

/// 斷言型別檢查產生至少一個含有指定子字串的錯誤。
fn assert_has_error(source: &str, expected_substr: &str) {
    let errors = type_errors(source);
    assert!(
        !errors.is_empty(),
        "預期型別檢查失敗，但未得到任何錯誤"
    );
    let found = errors.iter().any(|e| e.contains(expected_substr));
    assert!(
        found,
        "預期錯誤包含 {:?}，但實際錯誤為：{:?}",
        expected_substr,
        errors
    );
}

// ─── let 宣告型別不符 ─────────────────────────────────────────────────────────

#[test]
fn let_type_mismatch_caught_at_compile_time() {
    // let x: int = "hello" 應在執行前被捕捉
    assert_has_error(
        r#"let x: int = "hello";"#,
        "str",
    );
}

#[test]
fn let_bool_assigned_int_caught() {
    assert_has_error(
        r#"let flag: bool = 42;"#,
        "int",
    );
}

#[test]
fn let_str_assigned_bool_caught() {
    assert_has_error(
        r#"let name: str = true;"#,
        "bool",
    );
}

#[test]
fn let_correct_int_passes() {
    assert_passes(r#"let x: int = 42;"#);
}

#[test]
fn let_correct_str_passes() {
    assert_passes(r#"let s: str = "hello";"#);
}

#[test]
fn let_correct_bool_passes() {
    assert_passes(r#"let b: bool = true;"#);
}

// ─── 函式參數型別不符 ─────────────────────────────────────────────────────────

#[test]
fn function_param_type_mismatch_caught() {
    // foo 宣告 x: int，但呼叫時傳入字串
    assert_has_error(
        r#"
        fn foo(x: int) { print(x); }
        foo("hello");
        "#,
        "str",
    );
}

#[test]
fn function_param_bool_mismatch_caught() {
    assert_has_error(
        r#"
        fn check(flag: bool) { print(flag); }
        check(0);
        "#,
        "int",
    );
}

#[test]
fn function_correct_params_pass() {
    assert_passes(
        r#"
        fn add(a: int, b: int) { print(a + b); }
        add(3, 4);
        "#,
    );
}

// ─── 函式回傳型別不符 ─────────────────────────────────────────────────────────

#[test]
fn function_return_type_mismatch_caught() {
    // 宣告 -> int 但回傳字串
    assert_has_error(
        r#"
        fn get_name() -> int {
            return "Alice";
        }
        "#,
        "str",
    );
}

#[test]
fn function_return_bool_but_int_caught() {
    assert_has_error(
        r#"
        fn is_valid() -> bool {
            return 1;
        }
        "#,
        "int",
    );
}

#[test]
fn function_correct_return_passes() {
    assert_passes(
        r#"
        fn double(n: int) -> int {
            return n * 2;
        }
        "#,
    );
}

// ─── 二元運算型別不相容 ───────────────────────────────────────────────────────

#[test]
fn binary_int_plus_str_caught() {
    // int 和 str 不能相加
    assert_has_error(
        r#"
        let x: int = 1;
        let y: str = "a";
        let z = x + y;
        "#,
        "+",
    );
}

#[test]
fn binary_sub_with_str_caught() {
    assert_has_error(
        r#"
        let x: int = 5;
        let y: str = "3";
        let z = x - y;
        "#,
        "-",
    );
}

#[test]
fn binary_int_operations_pass() {
    // 正確的 int 運算應通過
    assert_passes(
        r#"
        let a: int = 10;
        let b: int = 3;
        let c = a + b;
        let d = a - b;
        let e = a * b;
        "#,
    );
}

#[test]
fn binary_str_concat_passes() {
    // 字串相加（拼接）應通過
    assert_passes(
        r#"
        let a: str = "hello";
        let b: str = " world";
        let c = a + b;
        "#,
    );
}

// ─── 一次收集多個錯誤 ─────────────────────────────────────────────────────────

#[test]
fn multiple_errors_collected_in_one_pass() {
    // 這段程式碼有兩個獨立的型別錯誤，應一次全部收集
    let source = r#"
        let x: int = "wrong1";
        let y: bool = "wrong2";
    "#;
    let errors = type_errors(source);
    assert!(
        errors.len() >= 2,
        "應收集到至少 2 個錯誤，實際得到 {} 個：{:?}",
        errors.len(),
        errors
    );
}

#[test]
fn three_errors_in_one_pass() {
    let source = r#"
        fn foo(x: int) -> bool {
            return "bad_return";
        }
        foo("bad_arg");
        let z: int = true;
    "#;
    let errors = type_errors(source);
    assert!(
        errors.len() >= 2,
        "應收集到至少 2 個錯誤，實際得到 {} 個：{:?}",
        errors.len(),
        errors
    );
}

// ─── 無型別標記的程式碼不受影響 ───────────────────────────────────────────────

#[test]
fn untyped_code_passes_without_regression() {
    // 完全沒有型別標記，型別檢查不應干擾
    assert_passes(
        r#"
        let x = 42;
        let y = "hello";
        let z = x + 1;
        fn greet(name) {
            print("Hello " + name);
        }
        greet("World");
        "#,
    );
}

#[test]
fn mixed_typed_and_untyped_passes() {
    assert_passes(
        r#"
        let a: int = 10;
        let b = a + 5;
        fn compute(n: int) {
            let result = n * 2;
            print(result);
        }
        compute(a);
        "#,
    );
}

// ─── 正確的型別標記程式碼通過檢查 ─────────────────────────────────────────────

#[test]
fn fully_typed_correct_program_passes() {
    assert_passes(
        r#"
        fn add(a: int, b: int) -> int {
            return a + b;
        }
        fn greet(name: str) -> str {
            return "Hello, " + name;
        }
        let sum: int = add(3, 4);
        let msg: str = greet("tiny");
        print(sum);
        print(msg);
        "#,
    );
}

#[test]
fn nested_function_types_pass() {
    assert_passes(
        r#"
        fn is_positive(n: int) -> bool {
            return n > 0;
        }
        fn describe(n: int) -> str {
            if is_positive(n) {
                return "positive";
            } else {
                return "non-positive";
            }
        }
        print(describe(5));
        "#,
    );
}

// ─── 賦值型別不符 ─────────────────────────────────────────────────────────────

#[test]
fn reassignment_type_mismatch_caught() {
    // 先宣告 int，再賦值 str 應報錯
    assert_has_error(
        r#"
        let x: int = 1;
        x = "oops";
        "#,
        "str",
    );
}

#[test]
fn reassignment_same_type_passes() {
    assert_passes(
        r#"
        let x: int = 1;
        x = 99;
        "#,
    );
}

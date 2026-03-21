//! Struct 測試：確認結構體定義、建立、欄位存取、欄位修改與型別檢查。

use tiny_lang::ast::Program;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;

fn run_ok(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

fn run_err(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    let mut interpreter = Interpreter::with_output(&mut output);
    interpreter.interpret(&program).unwrap_err().to_string()
}

#[test]
fn struct_definition_and_initialization() {
    let output = run_ok(
        r#"
        struct Point { x: int, y: int }
        let p = Point { x: 10, y: 20 };
        print(p.x);
        print(p.y);
    "#,
    );

    assert_eq!(output, "10\n20\n");
}

#[test]
fn struct_field_read_and_write() {
    let output = run_ok(
        r#"
        struct Point { x: int, y: int }
        let p = Point { x: 1, y: 2 };
        p.x = 30;
        print(p.x);
    "#,
    );

    assert_eq!(output, "30\n");
}

#[test]
fn struct_as_function_parameter() {
    let output = run_ok(
        r#"
        struct Point { x: int, y: int }
        fn sum(point: Point) -> int {
            return point.x + point.y;
        }
        let p = Point { x: 3, y: 4 };
        print(sum(p));
    "#,
    );

    assert_eq!(output, "7\n");
}

#[test]
fn struct_field_type_check() {
    let err = run_err(
        r#"
        struct Point { x: int, y: int }
        let p = Point { x: 1, y: 2 };
        p.x = "bad";
    "#,
    );

    assert!(err.contains("Expected int, got str"));
}

#[test]
fn struct_missing_field_errors() {
    let err = run_err(
        r#"
        struct Point { x: int, y: int }
        let p = Point { x: 1 };
    "#,
    );

    assert!(err.contains("missing field 'y'"));
}

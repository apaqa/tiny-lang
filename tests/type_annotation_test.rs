//! 型別標註測試。

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
fn type_annotation_matches_pass() {
    let output = run_ok(
        r#"
        let x: int = 10;
        let name: str = "hello";
        let arr: [int] = [1, 2, 3];
        let scores: {int} = {"math": 90, "eng": 88};
        print(x);
        print(name);
        print(len(arr));
        print(scores["math"]);
    "#,
    );

    assert_eq!(output, "10\nhello\n3\n90\n");
}

#[test]
fn type_annotation_mismatch_errors() {
    let err = run_err(r#"let x: int = "oops";"#);
    assert!(err.contains("Expected int, got str"));
}

#[test]
fn function_parameter_type_check() {
    let err = run_err(
        r#"
        fn add_one(x: int) -> int { return x + 1; }
        print(add_one("bad"));
    "#,
    );

    assert!(err.contains("Expected int, got str"));
}

#[test]
fn function_return_type_check() {
    let err = run_err(
        r#"
        fn wrong() -> int { return "bad"; }
        print(wrong());
    "#,
    );

    assert!(err.contains("Expected int, got str"));
}

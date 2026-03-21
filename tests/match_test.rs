//! Match 測試：確認 pattern matching、wildcard、識別字綁定與錯誤情況。

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
fn basic_match_int() {
    let output = run_ok(
        r#"
        let x = 2;
        match x {
            1 => { print("one"); }
            2 => { print("two"); }
            _ => { print("other"); }
        }
    "#,
    );

    assert_eq!(output, "two\n");
}

#[test]
fn match_string() {
    let output = run_ok(
        r#"
        let x = "tiny";
        match x {
            "lang" => { print("lang"); }
            "tiny" => { print("tiny"); }
            _ => { print("other"); }
        }
    "#,
    );

    assert_eq!(output, "tiny\n");
}

#[test]
fn wildcard_matches_anything() {
    let output = run_ok(
        r#"
        let x = true;
        match x {
            false => { print("false"); }
            _ => { print("fallback"); }
        }
    "#,
    );

    assert_eq!(output, "fallback\n");
}

#[test]
fn no_match_arm_reports_error() {
    let err = run_err(
        r#"
        let x = 99;
        match x {
            1 => { print("one"); }
            2 => { print("two"); }
        }
    "#,
    );

    assert!(err.contains("did not match any arm"));
}

#[test]
fn match_identifier_binding() {
    let output = run_ok(
        r#"
        let x = "hello";
        match x {
            value => { print(value); }
        }
    "#,
    );

    assert_eq!(output, "hello\n");
}

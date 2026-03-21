//! Result / Option 內建 enum 測試
use tiny_lang::interpreter::Interpreter;
use tiny_lang::{parse_source, type_check};

fn run_ok(source: &str) -> String {
    let program = parse_source(source).unwrap();
    type_check(&program).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn option_some_and_none_can_be_created() {
    let output = run_ok(
        r#"
        let some: Option<int> = Option::Some(42);
        let none: Option<int> = Option::None;
        print(some);
        print(none);
    "#,
    );

    assert!(output.contains("Option::Some"));
    assert!(output.contains("42"));
    assert!(output.contains("Option::None"));
}

#[test]
fn result_ok_and_err_can_be_created() {
    let output = run_ok(
        r#"
        let ok: Result<int, str> = Result::Ok(10);
        let err: Result<int, str> = Result::Err("oops");
        print(ok);
        print(err);
    "#,
    );

    assert!(output.contains("Result::Ok"));
    assert!(output.contains("10"));
    assert!(output.contains("Result::Err"));
    assert!(output.contains("oops"));
}

#[test]
fn match_can_destructure_option() {
    let output = run_ok(
        r#"
        let opt: Option<int> = Option::Some(42);
        match opt {
            Option::Some(value) => { print(value); }
            Option::None => { print("nothing"); }
        }
    "#,
    );

    assert_eq!(output, "42\n");
}

#[test]
fn match_can_destructure_result() {
    let output = run_ok(
        r#"
        let result: Result<int, str> = Result::Err("oops");
        match result {
            Result::Ok(value) => { print(value); }
            Result::Err(message) => { print(message); }
        }
    "#,
    );

    assert_eq!(output, "oops\n");
}

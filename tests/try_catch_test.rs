//! try/catch 錯誤處理測試。

use tiny_lang::ast::Program;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;

fn run_and_capture(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn try_catch_basic_usage() {
    let output = run_and_capture(
        r#"
        try {
            print(1 / 0);
        } catch e {
            print("caught");
        }
    "#,
    );

    assert_eq!(output, "caught\n");
}

#[test]
fn catch_receives_error_message() {
    let output = run_and_capture(
        r#"
        try {
            let arr = [1];
            print(arr[9]);
        } catch e {
            print(e);
        }
    "#,
    );

    assert!(output.contains("Runtime error: Index out of bounds: array length is 1, index is 9"));
}

#[test]
fn catch_is_skipped_when_no_error() {
    let output = run_and_capture(
        r#"
        try {
            print("safe");
        } catch e {
            print("bad");
        }
    "#,
    );

    assert_eq!(output, "safe\n");
}

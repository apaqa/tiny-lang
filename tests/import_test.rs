//! import 模組系統測試。

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
fn import_file_exposes_functions() {
    let output = run_and_capture(
        r#"
        import "examples/math_lib.tiny";
        print(square(5));
        print(PI);
    "#,
    );

    assert_eq!(output, "25\n3\n");
}

#[test]
fn duplicate_import_is_safe() {
    let output = run_and_capture(
        r#"
        import "examples/math_lib.tiny";
        import "examples/math_lib.tiny";
        print(square(3));
    "#,
    );

    assert_eq!(output, "9\n");
}

#[test]
fn import_missing_file_errors() {
    let program: Program = parse_source(r#"import "examples/missing.tiny";"#).unwrap();
    let mut output = Vec::new();
    let mut interpreter = Interpreter::with_output(&mut output);
    let err = interpreter.interpret(&program).unwrap_err().to_string();

    assert!(err.contains("Import file not found"));
}

use tiny_lang::ast::Program;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;

#[test]
fn parse_error_has_line_and_column() {
    let err = parse_source(
        "let x = [1,\n2,];",
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("[line 2, col"));
    assert!(err.contains("Parser error"));
}

#[test]
fn runtime_type_error_is_clear() {
    let program: Program = parse_source(
        r#"
        print(1 + "x");
    "#,
    )
    .unwrap();

    let mut output = Vec::new();
    let mut interpreter = Interpreter::with_output(&mut output);
    let err = interpreter.interpret(&program).unwrap_err().to_string();

    assert!(err.contains("Cannot add Int and String"));
}

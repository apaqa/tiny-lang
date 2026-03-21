use std::io::Cursor;

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
fn builtin_str_int_and_type_of() {
    let output = run_and_capture(
        r#"
        print(str(123));
        print(int("42"));
        print(int(true));
        print(type_of([1, 2]));
        print(type_of("hi"));
    "#,
    );

    assert_eq!(output, "123\n42\n1\narray\nstring\n");
}

#[test]
fn builtin_input_reads_from_reader() {
    let program: Program = parse_source(
        r#"
        let name = input("name? ");
        print(name);
    "#,
    )
    .unwrap();

    let mut output = Vec::new();
    let input = Cursor::new(b"tiny-lang\n".to_vec());
    let mut interpreter = Interpreter::with_io(&mut output, input);
    interpreter.interpret(&program).unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(rendered, "name? tiny-lang\n");
}

#[test]
fn builtin_argument_count_error() {
    let program: Program = parse_source("print(len(1, 2));").unwrap();
    let mut output = Vec::new();
    let mut interpreter = Interpreter::with_output(&mut output);
    let err = interpreter.interpret(&program).unwrap_err().to_string();

    assert!(err.contains("Function 'len' expects 1 arguments, got 2"));
}

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
fn string_concat() {
    let output = run_and_capture(
        r#"
        let s = "hello" + " " + "world";
        print(s);
    "#,
    );

    assert_eq!(output, "hello world\n");
}

#[test]
fn string_index() {
    let output = run_and_capture(
        r#"
        let s = "hello";
        print(s[0]);
        print(s[4]);
    "#,
    );

    assert_eq!(output, "h\no\n");
}

#[test]
fn string_compare() {
    let output = run_and_capture(
        r#"
        print("abc" == "abc");
        print("abc" != "xyz");
    "#,
    );

    assert_eq!(output, "true\ntrue\n");
}

#[test]
fn len_on_string() {
    let output = run_and_capture(
        r#"
        print(len("hello"));
    "#,
    );

    assert_eq!(output, "5\n");
}

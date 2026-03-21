//! 閉包建立、捕獲與高階函式測試。

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
fn closure_create_and_call() {
    let output = run_and_capture(
        r#"
        let double = fn(x) { return x * 2; };
        print(double(4));

        let triple = |x| x * 3;
        print(triple(4));
    "#,
    );

    assert_eq!(output, "8\n12\n");
}

#[test]
fn closure_captures_outer_variable() {
    let output = run_and_capture(
        r#"
        let factor = 5;
        let scale = |x| x * factor;
        print(scale(3));
    "#,
    );

    assert_eq!(output, "15\n");
}

#[test]
fn closure_passed_as_argument() {
    let output = run_and_capture(
        r#"
        fn apply_twice(f, value) {
            return f(f(value));
        }

        let inc = |x| x + 1;
        print(apply_twice(inc, 3));
    "#,
    );

    assert_eq!(output, "5\n");
}

//! for 迴圈與 break/continue 測試。

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
fn for_in_array() {
    let output = run_and_capture(
        r#"
        for x in [1, 2, 3] {
            print(x);
        }
    "#,
    );

    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn for_in_range() {
    let output = run_and_capture(
        r#"
        for i in range(2, 5) {
            print(i);
        }
    "#,
    );

    assert_eq!(output, "2\n3\n4\n");
}

#[test]
fn for_with_break() {
    let output = run_and_capture(
        r#"
        for i in range(0, 5) {
            if i == 3 {
                break;
            }
            print(i);
        }
    "#,
    );

    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn for_with_continue() {
    let output = run_and_capture(
        r#"
        for i in range(0, 5) {
            if i == 2 {
                continue;
            }
            print(i);
        }
    "#,
    );

    assert_eq!(output, "0\n1\n3\n4\n");
}

#[test]
fn nested_for_loops() {
    let output = run_and_capture(
        r#"
        for i in range(0, 2) {
            for j in range(0, 2) {
                print(str(i) + str(j));
            }
        }
    "#,
    );

    assert_eq!(output, "00\n01\n10\n11\n");
}

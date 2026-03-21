//! Iterator protocol 測試
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
fn struct_with_next_can_be_used_in_for_in() {
    let output = run_ok(
        r#"
        struct Range { current: int, end: int }

        fn Range.next(self) -> int {
            if self.current >= self.end {
                return null;
            }
            let value = self.current;
            self.current = self.current + 1;
            return value;
        }

        for i in Range { current: 0, end: 3 } {
            print(i);
        }
    "#,
    );

    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn iterator_stops_when_next_returns_null() {
    let output = run_ok(
        r#"
        struct Once { done: bool }

        fn Once.next(self) -> int {
            if self.done {
                return null;
            }
            self.done = true;
            return 42;
        }

        for value in Once { done: false } {
            print(value);
        }
    "#,
    );

    assert_eq!(output, "42\n");
}

#[test]
fn struct_without_next_errors_in_for_in() {
    let err = run_err(
        r#"
        struct Point { x: int }

        for item in Point { x: 1 } {
            print(item);
        }
    "#,
    );

    assert!(err.contains("has no next() method"));
}

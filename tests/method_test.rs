//! Method 測試：確認方法宣告、self、參數與回傳值。

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

#[test]
fn method_definition_and_call() {
    let output = run_ok(
        r#"
        struct Point { x: int, y: int }
        fn Point.sum() -> int {
            return self.x + self.y;
        }
        let p = Point { x: 3, y: 4 };
        print(p.sum());
    "#,
    );

    assert_eq!(output, "7\n");
}

#[test]
fn method_self_access() {
    let output = run_ok(
        r#"
        struct Point { x: int, y: int }
        fn Point.show_x() -> int {
            return self.x;
        }
        let p = Point { x: 9, y: 1 };
        print(p.show_x());
    "#,
    );

    assert_eq!(output, "9\n");
}

#[test]
fn method_accepts_parameters() {
    let output = run_ok(
        r#"
        struct Point { x: int, y: int }
        fn Point.distance(other: Point) -> int {
            let dx = self.x - other.x;
            let dy = self.y - other.y;
            return dx * dx + dy * dy;
        }
        let p = Point { x: 1, y: 1 };
        let q = Point { x: 4, y: 5 };
        print(p.distance(q));
    "#,
    );

    assert_eq!(output, "25\n");
}

#[test]
fn method_returns_value() {
    let output = run_ok(
        r#"
        struct Counter { value: int }
        fn Counter.inc(amount: int) -> int {
            self.value = self.value + amount;
            return self.value;
        }
        let c = Counter { value: 10 };
        print(c.inc(5));
        print(c.value);
    "#,
    );

    assert_eq!(output, "15\n15\n");
}

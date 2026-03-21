//! VM 測試：確認 bytecode VM 可正確執行語言特性，且輸出與 interpreter 一致。

use tiny_lang::ast::Program;
use tiny_lang::compiler::Compiler;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;
use tiny_lang::vm::VM;

fn run_interpreter_capture(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

fn run_vm_capture(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();
    let mut output = Vec::new();
    {
        let mut vm = VM::with_output(&mut output);
        vm.run(chunk).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn vm_runs_arithmetic() {
    let output = run_vm_capture(
        r#"
        print(1 + 2 * 3);
        print((10 - 4) / 2);
    "#,
    );

    assert_eq!(output, "7\n3\n");
}

#[test]
fn vm_runs_if_else() {
    let output = run_vm_capture(
        r#"
        let x = 10;
        if x > 5 {
            print("big");
        } else {
            print("small");
        }
    "#,
    );

    assert_eq!(output, "big\n");
}

#[test]
fn vm_runs_while_loop() {
    let output = run_vm_capture(
        r#"
        let i = 0;
        while i < 3 {
            print(i);
            i = i + 1;
        }
    "#,
    );

    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn vm_runs_for_loop() {
    let output = run_vm_capture(
        r#"
        for x in range(0, 3) {
            print(x);
        }
    "#,
    );

    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn vm_runs_function_call_and_recursion() {
    let output = run_vm_capture(
        r#"
        fn fib(n: int) -> int {
            if n < 2 {
                return n;
            }
            return fib(n - 1) + fib(n - 2);
        }
        print(fib(8));
    "#,
    );

    assert_eq!(output, "21\n");
}

#[test]
fn vm_runs_array_and_map() {
    let output = run_vm_capture(
        r#"
        let arr = [1, 2, 3];
        arr[1] = 99;
        print(arr[1]);

        let m = {"name": "tiny"};
        m["name"] = "lang";
        print(m["name"]);
    "#,
    );

    assert_eq!(output, "99\nlang\n");
}

#[test]
fn vm_runs_struct_and_method() {
    let output = run_vm_capture(
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
        p.x = 7;
        print(p.x);
    "#,
    );

    assert_eq!(output, "25\n7\n");
}

#[test]
fn vm_matches_interpreter_output() {
    let source = r#"
        struct Point { x: int, y: int }
        fn Point.sum() -> int {
            return self.x + self.y;
        }

        fn fact(n: int) -> int {
            if n == 0 {
                return 1;
            }
            return n * fact(n - 1);
        }

        let p = Point { x: 3, y: 4 };
        let total = 0;
        let arr = [1, 2, 3];
        arr[0] = 9;

        print(p.sum());
        print(fact(5));

        match arr[0] {
            9 => { print("match"); }
            _ => { print("other"); }
        }

        for x in range(0, 3) {
            print(x);
        }
    "#;

    let interpreter_output = run_interpreter_capture(source);
    let vm_output = run_vm_capture(source);
    assert_eq!(vm_output, interpreter_output);
}

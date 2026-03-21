//! VM / interpreter 一致性測試。

use tiny_lang::ast::Program;
use tiny_lang::compiler::Compiler;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;
use tiny_lang::vm::VM;

fn run_interpreter(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

fn run_vm(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();
    let mut output = Vec::new();
    {
        let mut vm = VM::with_output(&mut output);
        vm.run(chunk).unwrap();
    }
    String::from_utf8(output).unwrap()
}

fn assert_parity(source: &str) {
    let interpreter_output = run_interpreter(source);
    let vm_output = run_vm(source);
    assert_eq!(vm_output, interpreter_output);
}

#[test]
fn parity_arithmetic() {
    assert_parity("print(1 + 2 * 3); print(9 % 4);");
}

#[test]
fn parity_if_else() {
    assert_parity(r#"let x = 7; if x > 5 { print("big"); } else { print("small"); }"#);
}

#[test]
fn parity_while_loop() {
    assert_parity("let i = 0; while i < 3 { print(i); i = i + 1; }");
}

#[test]
fn parity_for_loop() {
    assert_parity("for x in range(0, 3) { print(x); }");
}

#[test]
fn parity_function_call() {
    assert_parity("fn add(a: int, b: int) -> int { return a + b; } print(add(3, 4));");
}

#[test]
fn parity_recursion() {
    assert_parity(
        "fn fact(n: int) -> int { if n == 0 { return 1; } return n * fact(n - 1); } print(fact(5));",
    );
}

#[test]
fn parity_closure_return_value() {
    assert_parity(
        "fn make_adder(n) { let f = |x| { return x + n; }; return f; } let add5 = make_adder(5); print(add5(8));",
    );
}

#[test]
fn parity_shared_upvalue() {
    assert_parity(
        "fn make_pair() { let x = 0; let a = || { x = x + 1; return x; }; let b = || { x = x + 10; return x; }; print(a()); print(b()); print(a()); } make_pair();",
    );
}

#[test]
fn parity_nested_closure() {
    assert_parity(
        "fn outer(a) { let mid = |b| { let inner = |c| { return a + b + c; }; return inner; }; return mid; } let f = outer(2); let g = f(3); print(g(4));",
    );
}

#[test]
fn parity_loop_capture() {
    assert_parity(
        "let fs = []; for x in range(0, 3) { let f = || { return x; }; push(fs, f); } print(fs[0]()); print(fs[1]()); print(fs[2]());",
    );
}

#[test]
fn parity_array() {
    assert_parity("let arr = [1, 2, 3]; arr[1] = 9; print(arr[1]);");
}

#[test]
fn parity_map() {
    assert_parity(r#"let m = {"name": "tiny"}; m["name"] = "lang"; print(m["name"]);"#);
}

#[test]
fn parity_struct() {
    assert_parity(
        "struct Point { x: int, y: int } fn Point.sum() -> int { return self.x + self.y; } let p = Point { x: 2, y: 5 }; print(p.sum());",
    );
}

#[test]
fn parity_match() {
    assert_parity(r#"let x = 2; match x { 1 => { print("one"); } 2 => { print("two"); } _ => { print("other"); } }"#);
}

#[test]
fn parity_enum() {
    assert_parity(
        "enum Result { Ok { value: int }, Err { message: str } } let v = Result::Ok { value: 7 }; match v { Result::Ok(value) => { print(value.value); } _ => { print(0); } }",
    );
}

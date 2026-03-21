//! VM try/catch 測試。

use tiny_lang::ast::Program;
use tiny_lang::compiler::Compiler;
use tiny_lang::parse_source;
use tiny_lang::vm::VM;

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

#[test]
fn vm_try_catch_basic_usage() {
    let output = run_vm(
        r#"
        try {
            print(1 / 0);
        } catch e {
            print("caught");
        }
    "#,
    );

    assert_eq!(output, "caught\n");
}

#[test]
fn vm_catch_receives_error_message() {
    let output = run_vm(
        r#"
        try {
            let arr = [1];
            print(arr[9]);
        } catch e {
            print(e);
        }
    "#,
    );

    assert!(output.contains("Runtime error: Index out of bounds: array length is 1, index is 9"));
}

#[test]
fn vm_nested_try_catch() {
    let output = run_vm(
        r#"
        try {
            try {
                print(1 / 0);
            } catch inner {
                print("inner");
            }
        } catch outer {
            print("outer");
        }
    "#,
    );

    assert_eq!(output, "inner\n");
}

#[test]
fn vm_try_catches_function_runtime_error() {
    let output = run_vm(
        r#"
        fn boom() {
            print(1 / 0);
        }

        try {
            boom();
        } catch e {
            print(e);
        }
    "#,
    );

    assert!(output.contains("Runtime error: Cannot divide by zero"));
}

//! VM 閉包測試：確認 bytecode VM 在 VM 模式下能正確執行閉包。
//!
//! 測試項目：
//! - 閉包捕獲外部變數（upvalue）
//! - 閉包作為值傳遞和呼叫
//! - 閉包在多次呼叫間維持狀態（counter 模式）

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
fn vm_closure_captures_global_variable() {
    // 外部 global 變數可在 lambda 中直接存取（透過 GetGlobal，不需要 upvalue）
    let output = run_vm(
        r#"
        let factor = 10;
        let scale = |x| { return x * factor; };
        print(scale(3));
        print(scale(5));
    "#,
    );
    assert_eq!(output, "30\n50\n");
}

#[test]
fn vm_closure_captures_local_upvalue() {
    // 閉包捕獲函式內部的 local 變數（upvalue）
    let output = run_vm(
        r#"
        fn make_adder(n) {
            let adder = |x| { return x + n; };
            return adder;
        }
        let add5 = make_adder(5);
        let add10 = make_adder(10);
        print(add5(3));
        print(add10(3));
    "#,
    );
    assert_eq!(output, "8\n13\n");
}

#[test]
fn vm_closure_counter_maintains_state() {
    // counter 模式：閉包透過 upvalue 在多次呼叫間維持可變狀態
    let output = run_vm(
        r#"
        fn make_counter() {
            let count = 0;
            let inc = || {
                count = count + 1;
                return count;
            };
            return inc;
        }
        let c = make_counter();
        print(c());
        print(c());
        print(c());
    "#,
    );
    assert_eq!(output, "1\n2\n3\n");
}

#[test]
fn vm_closure_passed_as_argument() {
    // 閉包可作為引數傳入高階函式
    let output = run_vm(
        r#"
        fn apply(f, x) {
            return f(x);
        }
        let double = |n| { return n * 2; };
        print(apply(double, 7));
        print(apply(double, 3));
    "#,
    );
    assert_eq!(output, "14\n6\n");
}

#[test]
fn vm_closure_no_captures_is_plain_function() {
    // 無捕獲的 lambda 應當作普通函式執行
    let output = run_vm(
        r#"
        let square = |x| { return x * x; };
        print(square(4));
        print(square(9));
    "#,
    );
    assert_eq!(output, "16\n81\n");
}

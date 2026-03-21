//! Compiler 測試：確認 AST 會被編譯成正確的 bytecode 形狀。

use tiny_lang::compiler::{Compiler, OpCode, disassemble};
use tiny_lang::environment::Value;
use tiny_lang::parse_source;

#[test]
fn compile_simple_expression_emits_arithmetic_bytecode() {
    let program = parse_source(r#"print(1 + 2);"#).unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();

    assert_eq!(
        &chunk.code[..5],
        &[
            OpCode::Constant(0),
            OpCode::Constant(1),
            OpCode::Add,
            OpCode::Print,
            OpCode::Halt,
        ]
    );
    assert_eq!(chunk.constants[0], Value::Int(1));
    assert_eq!(chunk.constants[1], Value::Int(2));
}

#[test]
fn compile_if_else_emits_jump_instructions() {
    let program = parse_source(
        r#"
        if true {
            print(1);
        } else {
            print(2);
        }
    "#,
    )
    .unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();

    assert!(chunk.code.iter().any(|op| matches!(op, OpCode::JumpIfFalse(_))));
    assert!(chunk.code.iter().any(|op| matches!(op, OpCode::Jump(_))));
}

#[test]
fn compile_function_emits_call_and_return() {
    let program = parse_source(
        r#"
        fn add(a: int, b: int) -> int {
            return a + b;
        }
        print(add(1, 2));
    "#,
    )
    .unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();

    assert!(chunk.code.iter().any(|op| matches!(op, OpCode::Call(2))));

    let has_return = chunk.constants.iter().any(|value| match value {
        Value::CompiledFunction(function) => function.chunk.code.iter().any(|op| matches!(op, OpCode::Return)),
        _ => false,
    });
    assert!(has_return);
}

#[test]
fn disassemble_formats_bytecode_lines() {
    let program = parse_source(r#"print(1 + 2);"#).unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();
    let rendered = disassemble(&chunk);

    assert!(rendered.contains("0000"));
    assert!(rendered.contains("Constant"));
    assert!(rendered.contains("Add"));
    assert!(rendered.contains("Print"));
}

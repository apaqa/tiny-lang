//! tiny-lang 對外 API。

pub mod ast;
pub mod compiler;
pub mod environment;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod vm;

use std::path::Path;

use ast::Program;
use error::Result;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;
use vm::VM;

/// 把原始碼 parse 成 AST。
pub fn parse_source(source: &str) -> Result<Program> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_with_spans()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// 直接執行一段原始碼。
pub fn run_source(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    let mut interpreter = Interpreter::new();
    interpreter.interpret(&program)
}

/// 使用 bytecode compiler + VM 執行原始碼。
pub fn compile_and_run(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    let chunk = compiler::Compiler::compile_program(&program)?;
    let mut vm = VM::new();
    vm.run(chunk)?;
    Ok(())
}

/// 執行檔案，支援 import。
pub fn run_file(path: impl AsRef<Path>) -> Result<()> {
    let mut interpreter = Interpreter::new();
    interpreter.interpret_file(path)
}

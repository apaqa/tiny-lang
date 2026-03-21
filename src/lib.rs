//! tiny-lang 對外 API。

pub mod ast;
pub mod compiler;
pub mod environment;
pub mod error;
pub mod formatter;
pub mod gc;
pub mod interpreter;
pub mod lexer;
pub mod lsp;
pub mod parser;
pub mod token;
pub mod typechecker;
pub mod vm;

use std::path::Path;

use ast::Program;
use error::{Result, TinyLangError};
use formatter::format_program;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;
use typechecker::TypeChecker;
use vm::VM;

/// 把原始碼 parse 成 AST。
pub fn parse_source(source: &str) -> Result<Program> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_with_spans()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// 對已解析的程式執行靜態型別檢查。
///
/// 若有型別錯誤則將所有錯誤整合成一個 `TinyLangError` 回傳，
/// 讓呼叫端可以一次看到所有問題。
pub fn type_check(program: &Program) -> Result<()> {
    let mut checker = TypeChecker::new();
    checker.check_program(program);
    if checker.errors.is_empty() {
        return Ok(());
    }
    // 將所有型別錯誤合併成一則訊息
    let messages: Vec<String> = checker.errors.iter().map(|e| e.message.clone()).collect();
    Err(TinyLangError::type_check(messages.join("\n")))
}

/// 直接執行一段原始碼（先進行靜態型別檢查）。
pub fn run_source(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    type_check(&program)?;
    let mut interpreter = Interpreter::new();
    interpreter.interpret(&program)
}

/// 使用 bytecode compiler + VM 執行原始碼（先進行靜態型別檢查）。
pub fn compile_and_run(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    type_check(&program)?;
    let chunk = compiler::Compiler::compile_program(&program)?;
    let mut vm = VM::new();
    vm.run(chunk)?;
    Ok(())
}

/// 中文註解：檔案模式下要把 import 的基準目錄設成來源檔所在資料夾。
pub fn compile_and_run_file(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path)?;
    let program = parse_source(&source)?;
    type_check(&program)?;
    let chunk = compiler::Compiler::compile_program(&program)?;
    let mut vm = VM::new();
    if let Some(parent) = path.parent() {
        vm.set_current_dir(parent);
    }
    vm.run(chunk)?;
    Ok(())
}

/// 中文註解：格式化單一程式字串。
pub fn format_source(source: &str) -> Result<String> {
    let program = parse_source(source)?;
    Ok(format_program(&program))
}

/// 執行檔案，支援 import（先進行靜態型別檢查）。
pub fn run_file(path: impl AsRef<Path>) -> Result<()> {
    // 先讀取並型別檢查，錯誤就提前回報
    let source = std::fs::read_to_string(path.as_ref())?;
    let program = parse_source(&source)?;
    type_check(&program)?;
    // 型別檢查通過後，透過 interpret_file 執行以保留 import 的目錄切換邏輯
    let mut interpreter = Interpreter::new();
    interpreter.interpret_file(path)
}

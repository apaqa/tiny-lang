//! tiny-lang 對外 API。
//!
//! 這裡提供 parse、直接執行字串，以及檔案模式的共用入口。

pub mod ast;
pub mod environment;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod token;

use std::path::Path;

use ast::Program;
use error::Result;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

/// 將 tiny-lang 原始碼解析成 AST。
pub fn parse_source(source: &str) -> Result<Program> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_with_spans()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// 直接執行一段 tiny-lang 原始碼。
pub fn run_source(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    let mut interpreter = Interpreter::new();
    interpreter.interpret(&program)
}

/// 以檔案模式執行程式，供 import 正確解析相對路徑。
pub fn run_file(path: impl AsRef<Path>) -> Result<()> {
    let mut interpreter = Interpreter::new();
    interpreter.interpret_file(path)
}

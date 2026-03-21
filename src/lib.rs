//! tiny-lang 公開 API。
//!
//! 編譯原理 Phase 1 的資料流：
//! 1. Lexer：把字串掃成 token
//! 2. Parser：把 token 組成 AST
//! 3. Interpreter：直接走訪 AST 並執行
//!
//! 這種直譯方式稱為 tree-walking interpreter。

pub mod ast;
pub mod environment;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod token;

use ast::Program;
use error::Result;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

pub fn parse_source(source: &str) -> Result<Program> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

pub fn run_source(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    let mut interpreter = Interpreter::new();
    interpreter.interpret(&program)
}

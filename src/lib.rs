//! tiny-lang 公開 API。
//!
//! Phase 2 的資料流：
//! 1. Lexer：字串 -> token（附帶位置）
//! 2. Parser：token -> AST
//! 3. Interpreter：沿著 AST 直接執行

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
    let tokens = lexer.tokenize_with_spans()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

pub fn run_source(source: &str) -> Result<()> {
    let program = parse_source(source)?;
    let mut interpreter = Interpreter::new();
    interpreter.interpret(&program)
}

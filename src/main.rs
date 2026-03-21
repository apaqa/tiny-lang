//! tiny-lang 的執行入口。
//!
//! `cargo run`
//! 會進入 REPL。
//!
//! `cargo run -- examples\\demo.tiny`
//! 會直接執行檔案。

use std::env;
use std::fs;
use std::io::{self, Write};

use tiny_lang::error::{Result, TinyLangError};
use tiny_lang::interpreter::Interpreter;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        return Err(TinyLangError::io("用法：cargo run -- [file.tiny]"));
    }

    if let Some(path) = args.get(1) {
        let source = fs::read_to_string(path)?;
        let mut interpreter = Interpreter::new();
        interpreter.interpret_source(&source)?;
    } else {
        repl()?;
    }

    Ok(())
}

fn repl() -> Result<()> {
    let stdin = io::stdin();
    let mut interpreter = Interpreter::new();
    let mut buffer = String::new();
    let mut brace_balance = 0_i32;

    loop {
        if brace_balance == 0 {
            print!("tiny> ");
        } else {
            print!("....> ");
        }
        io::stdout().flush()?;

        let mut line = String::new();
        let bytes = stdin.read_line(&mut line)?;
        if bytes == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        if brace_balance == 0 && (trimmed == ":quit" || trimmed == ":exit") {
            break;
        }
        if trimmed.is_empty() && brace_balance == 0 {
            continue;
        }

        brace_balance += line.chars().filter(|ch| *ch == '{').count() as i32;
        brace_balance -= line.chars().filter(|ch| *ch == '}').count() as i32;
        buffer.push_str(&line);

        let ready = brace_balance <= 0
            && (trimmed.ends_with(';') || trimmed.ends_with('}') || trimmed.is_empty());

        if ready {
            if let Err(err) = interpreter.interpret_source(&buffer) {
                eprintln!("{err}");
            }
            buffer.clear();
            brace_balance = 0;
        }
    }

    Ok(())
}

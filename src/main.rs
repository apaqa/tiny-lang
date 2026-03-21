//! tiny-lang 命令列入口。
//!
//! 預設使用 tree-walking interpreter。
//! `--vm` 會改用 bytecode compiler + VM。
//! `--disasm` 會印出 bytecode 反組譯結果。

use std::env;
use std::fs;
use std::io::{self, Write};

use tiny_lang::compiler::{Compiler, disassemble};
use tiny_lang::error::{Result, TinyLangError};
use tiny_lang::interpreter::Interpreter;
use tiny_lang::{compile_and_run, parse_source, run_file};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut use_vm = false;
    let mut show_disasm = false;
    let mut path: Option<String> = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--vm" => use_vm = true,
            "--disasm" => show_disasm = true,
            other if other.starts_with("--") => {
                return Err(TinyLangError::io(format!("unknown flag: {other}")));
            }
            other => {
                if path.is_some() {
                    return Err(TinyLangError::io(
                        "usage: cargo run -- [--vm] [--disasm] [file.tiny]",
                    ));
                }
                path = Some(other.to_string());
            }
        }
    }

    if let Some(path) = path {
        let source = fs::read_to_string(&path)?;
        if show_disasm {
            let program = parse_source(&source)?;
            let chunk = Compiler::compile_program(&program)?;
            println!("{}", disassemble(&chunk));
            if !use_vm {
                return Ok(());
            }
        }

        if use_vm {
            compile_and_run(&source)?;
        } else {
            run_file(path)?;
        }
    } else if show_disasm {
        return Err(TinyLangError::io("--disasm requires a file path"));
    } else if use_vm {
        repl_vm()?;
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

fn repl_vm() -> Result<()> {
    let stdin = io::stdin();
    let mut buffer = String::new();
    let mut brace_balance = 0_i32;

    loop {
        if brace_balance == 0 {
            print!("tiny(vm)> ");
        } else {
            print!("....(vm)> ");
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
            if let Err(err) = compile_and_run(&buffer) {
                eprintln!("{err}");
            }
            buffer.clear();
            brace_balance = 0;
        }
    }

    Ok(())
}

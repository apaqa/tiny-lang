//! VM import 測試。

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tiny_lang::ast::Program;
use tiny_lang::compiler::Compiler;
use tiny_lang::parse_source;
use tiny_lang::vm::VM;

fn unique_temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tiny_lang_vm_import_{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_vm_in_dir(source: &str, dir: &std::path::Path) -> String {
    let program: Program = parse_source(source).unwrap();
    let chunk = Compiler::compile_program(&program).unwrap();
    let mut output = Vec::new();
    {
        let mut vm = VM::with_output(&mut output);
        vm.set_current_dir(dir);
        vm.run(chunk).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn vm_imports_file() {
    let dir = unique_temp_dir();
    fs::write(
        dir.join("math_lib.tiny"),
        "fn square(n: int) -> int { return n * n; } let PI = 3;",
    )
    .unwrap();

    let output = run_vm_in_dir(
        r#"
        import "math_lib.tiny";
        print(square(5));
        print(PI);
    "#,
        &dir,
    );

    assert_eq!(output, "25\n3\n");
}

#[test]
fn vm_import_exposes_functions() {
    let dir = unique_temp_dir();
    fs::write(
        dir.join("lib.tiny"),
        "fn greet() -> int { return 42; }",
    )
    .unwrap();

    let output = run_vm_in_dir(
        r#"
        import "lib.tiny";
        print(greet());
    "#,
        &dir,
    );

    assert_eq!(output, "42\n");
}

#[test]
fn vm_duplicate_import_is_safe() {
    let dir = unique_temp_dir();
    fs::write(
        dir.join("lib.tiny"),
        "fn square(n: int) -> int { return n * n; }",
    )
    .unwrap();

    let output = run_vm_in_dir(
        r#"
        import "lib.tiny";
        import "lib.tiny";
        print(square(3));
    "#,
        &dir,
    );

    assert_eq!(output, "9\n");
}

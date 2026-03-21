//! Enum 測試：驗證 interpreter 與 VM 模式下 enum 的定義、建立與 match 解構。
//!
//! 測試項目：
//! - enum 定義與 variant 建立
//! - match 對 enum variant 的模式比對
//! - enum variant 攜帶欄位資料
//! - VM 模式下 enum 支援

use tiny_lang::ast::Program;
use tiny_lang::compiler::Compiler;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;
use tiny_lang::vm::VM;

fn run_interpreter(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interp = Interpreter::with_output(&mut output);
        interp.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

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

// ─── Interpreter 測試 ─────────────────────────────────────────────────────────

#[test]
fn interp_enum_define_and_create() {
    // enum 定義與 variant 建立，列印時顯示 EnumName::VariantName
    let output = run_interpreter(
        r#"
        enum Color { Red, Green, Blue }
        let c = Color::Green;
        print(c);
    "#,
    );
    assert_eq!(output, "Color::Green\n");
}

#[test]
fn interp_enum_match_variant() {
    // match 對 enum variant 的模式比對
    let output = run_interpreter(
        r#"
        enum Direction { North, South, East, West }
        let d = Direction::East;
        match d {
            Direction::North => { print("north"); }
            Direction::South => { print("south"); }
            Direction::East  => { print("east"); }
            Direction::West  => { print("west"); }
        }
    "#,
    );
    assert_eq!(output, "east\n");
}

#[test]
fn interp_enum_with_fields() {
    // enum variant 攜帶欄位資料
    let output = run_interpreter(
        r#"
        enum Shape { Circle { radius: int }, Square { side: int } }
        let s = Shape::Circle { radius: 7 };
        print(s);
    "#,
    );
    // print 應顯示 enum variant 及欄位
    assert!(output.contains("Circle"), "應包含 Circle");
    assert!(output.contains("7"), "應包含 radius 值 7");
}

#[test]
fn interp_enum_match_wildcard() {
    // match 中的 wildcard 能匹配未命名的 enum variant
    let output = run_interpreter(
        r#"
        enum Status { Ok, Err, Pending }
        let s = Status::Pending;
        match s {
            Status::Ok  => { print("ok"); }
            Status::Err => { print("err"); }
            _           => { print("other"); }
        }
    "#,
    );
    assert_eq!(output, "other\n");
}

// ─── VM 測試 ─────────────────────────────────────────────────────────────────

#[test]
fn vm_enum_define_and_create() {
    // VM 模式下 enum variant 建立與列印
    let output = run_vm(
        r#"
        enum Color { Red, Green, Blue }
        let c = Color::Blue;
        print(c);
    "#,
    );
    assert_eq!(output, "Color::Blue\n");
}

#[test]
fn vm_enum_match_variant() {
    // VM 模式下 match 對 enum variant 的模式比對
    let output = run_vm(
        r#"
        enum Direction { North, South, East, West }
        let d = Direction::South;
        match d {
            Direction::North => { print("north"); }
            Direction::South => { print("south"); }
            Direction::East  => { print("east"); }
            Direction::West  => { print("west"); }
        }
    "#,
    );
    assert_eq!(output, "south\n");
}

#[test]
fn vm_enum_with_fields() {
    // VM 模式下 enum variant 攜帶欄位資料
    let output = run_vm(
        r#"
        enum Shape { Circle { radius: int } }
        let s = Shape::Circle { radius: 5 };
        print(s);
    "#,
    );
    assert!(output.contains("Circle"), "應包含 Circle");
    assert!(output.contains("5"), "應包含 radius 值 5");
}

#[test]
fn vm_enum_match_wildcard() {
    // VM 模式下 wildcard 能匹配 enum variant
    let output = run_vm(
        r#"
        enum Status { Active, Inactive, Pending }
        let s = Status::Active;
        match s {
            Status::Inactive => { print("inactive"); }
            _                => { print("other"); }
        }
    "#,
    );
    assert_eq!(output, "other\n");
}

#[test]
fn vm_enum_multiple_variants() {
    // VM 模式下多個 enum variant 依序比對
    let output = run_vm(
        r#"
        enum Coin { Penny, Nickel, Dime }
        let a = Coin::Penny;
        let b = Coin::Dime;
        let c = Coin::Nickel;
        match a {
            Coin::Penny  => { print(1); }
            Coin::Nickel => { print(5); }
            Coin::Dime   => { print(10); }
        }
        match b {
            Coin::Penny  => { print(1); }
            Coin::Nickel => { print(5); }
            Coin::Dime   => { print(10); }
        }
        match c {
            Coin::Penny  => { print(1); }
            Coin::Nickel => { print(5); }
            Coin::Dime   => { print(10); }
        }
    "#,
    );
    assert_eq!(output, "1\n10\n5\n");
}

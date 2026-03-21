//! formatter 測試。

use tiny_lang::format_source;

#[test]
fn formatter_uses_four_space_indentation() {
    let formatted = format_source(
        r#"
        if true {
        print(1);
        }
    "#,
    )
    .unwrap();

    assert!(formatted.contains("if true {\n    print(1);\n}"));
}

#[test]
fn formatter_adds_blank_lines_around_functions() {
    let formatted = format_source(
        r#"
        let x = 1;
        fn add(a: int, b: int) -> int { return a + b; }
        let y = add(x, 2);
    "#,
    )
    .unwrap();

    assert!(formatted.contains("let x = 1;\n\nfn add(a: int, b: int) -> int {\n    return a + b;\n}\n\nlet y = add(x, 2);"));
}

#[test]
fn formatter_adds_spaces_around_binary_operators() {
    let formatted = format_source("let x=1+2*3;").unwrap();
    assert!(formatted.contains("let x = 1 + (2 * 3);") || formatted.contains("let x = 1 + 2 * 3;"));
}

#[test]
fn formatter_keeps_formatted_source_stable() {
    let source = "let x = 1;\n\nfn add(a: int, b: int) -> int {\n    return a + b;\n}\n";
    let formatted = format_source(source).unwrap();
    assert_eq!(formatted, source.trim_end_matches('\n'));
}

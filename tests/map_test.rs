//! Map 建立、索引與內建函式測試。

use tiny_lang::ast::Program;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;

fn run_and_capture(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn map_create_read_and_write() {
    let output = run_and_capture(
        r#"
        let m = {"name": "Alice", "age": 30};
        print(m["name"]);
        m["email"] = "alice@example.com";
        print(m["email"]);
    "#,
    );

    assert_eq!(output, "Alice\nalice@example.com\n");
}

#[test]
fn map_len_keys_and_values() {
    let output = run_and_capture(
        r#"
        let m = {"b": 2, "a": 1};
        print(len(m));
        print(keys(m));
        print(values(m));
    "#,
    );

    assert_eq!(output, "2\n[a, b]\n[1, 2]\n");
}

#[test]
fn map_as_function_argument() {
    let output = run_and_capture(
        r#"
        fn show_name(user) {
            print(user["name"]);
        }

        let user = {"name": "Tiny"};
        show_name(user);
    "#,
    );

    assert_eq!(output, "Tiny\n");
}

//! 標準庫函式測試。

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
fn math_functions_work() {
    let output = run_and_capture(
        r#"
        print(abs(-7));
        print(max(3, 9));
        print(min(3, 9));
        print(pow(2, 5));
    "#,
    );

    assert_eq!(output, "7\n9\n3\n32\n");
}

#[test]
fn string_functions_work() {
    let output = run_and_capture(
        r#"
        print(split("a,b,c", ","));
        print(join(["a", "b", "c"], "-"));
        print(trim("  hi  "));
        print(upper("tiny"));
        print(lower("TINY"));
        print(contains("hello", "ell"));
        print(replace("hello", "ll", "yy"));
    "#,
    );

    assert_eq!(output, "[a, b, c]\na-b-c\nhi\nTINY\ntiny\ntrue\nheyyo\n");
}

#[test]
fn array_higher_order_functions_work() {
    let output = run_and_capture(
        r#"
        let arr = [3, 1, 2];
        print(sort(arr));
        print(reverse(arr));
        print(map([1, 2, 3], |x| x * 2));
        print(filter([1, 2, 3, 4], |x| x % 2 == 0));
        print(reduce([1, 2, 3], |acc, x| acc + x, 0));
        print(find([1, 3, 4, 6], |x| x % 2 == 0));
    "#,
    );

    assert_eq!(output, "[1, 2, 3]\n[3, 2, 1]\n[2, 4, 6]\n[2, 4]\n6\n4\n");
}

#[test]
fn assert_success_and_failure() {
    let output = run_and_capture(
        r#"
        assert(true, "ok");
        try {
            assert(false, "boom");
        } catch e {
            print(e);
        }
    "#,
    );

    assert_eq!(output, "Runtime error: boom\n");
}

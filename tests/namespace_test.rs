//! 命名空間匯入（import ... as）測試。

use tiny_lang::interpreter::Interpreter;
use tiny_lang::{parse_source, type_check};

fn run_ok(source: &str) -> String {
    let program = parse_source(source).unwrap();
    type_check(&program).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn namespace_import_gcd_and_lcm() {
    let output = run_ok(
        r#"
        import "stdlib/math_ext.tiny" as math_ext;
        print(math_ext.gcd(12, 8));
        print(math_ext.lcm(4, 6));
    "#,
    );
    assert_eq!(output, "4\n12\n");
}

#[test]
fn namespace_import_factorial_and_fibonacci() {
    let output = run_ok(
        r#"
        import "stdlib/math_ext.tiny" as math_ext;
        print(math_ext.factorial(5));
        print(math_ext.fibonacci(10));
    "#,
    );
    assert_eq!(output, "120\n55\n");
}

#[test]
fn namespace_import_is_prime() {
    let output = run_ok(
        r#"
        import "stdlib/math_ext.tiny" as math_ext;
        print(math_ext.is_prime(7));
        print(math_ext.is_prime(9));
        print(math_ext.is_prime(2));
    "#,
    );
    assert_eq!(output, "true\nfalse\ntrue\n");
}

#[test]
fn two_namespaces_do_not_conflict() {
    let output = run_ok(
        r#"
        import "stdlib/math_ext.tiny" as math_ext;
        import "stdlib/collections.tiny" as coll;
        let s: array = coll.stack_new();
        s = coll.stack_push(s, math_ext.factorial(4));
        print(coll.stack_pop(s));
    "#,
    );
    assert_eq!(output, "24\n");
}

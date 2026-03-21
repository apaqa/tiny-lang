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
fn interpret_variable_declaration_and_usage() {
    let output = run_and_capture(
        r#"
        let x = 10;
        print(x);
    "#,
    );
    assert_eq!(output, "10\n");
}

#[test]
fn interpret_math_expression() {
    let output = run_and_capture(
        r#"
        let value = 1 + 2 * 3;
        print(value);
    "#,
    );
    assert_eq!(output, "7\n");
}

#[test]
fn interpret_if_else() {
    let output = run_and_capture(
        r#"
        let x = 10;
        if x > 5 { print("big"); } else { print("small"); }
    "#,
    );
    assert_eq!(output, "big\n");
}

#[test]
fn interpret_while_loop() {
    let output = run_and_capture(
        r#"
        let i = 0;
        while i < 3 {
            print(i);
            i = i + 1;
        }
    "#,
    );
    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn interpret_function_call() {
    let output = run_and_capture(
        r#"
        fn add(a, b) { return a + b; }
        let result = add(3, 4);
        print(result);
    "#,
    );
    assert_eq!(output, "7\n");
}

#[test]
fn interpret_recursive_function() {
    let output = run_and_capture(
        r#"
        fn fact(n) {
            if n == 0 {
                return 1;
            } else {
                return n * fact(n - 1);
            }
        }
        print(fact(5));
    "#,
    );
    assert_eq!(output, "120\n");
}

#[test]
fn interpret_scope_isolation() {
    let output = run_and_capture(
        r#"
        let x = 1;
        fn test() {
            let x = 99;
            print(x);
        }
        test();
        print(x);
    "#,
    );
    assert_eq!(output, "99\n1\n");
}

#[test]
fn interpret_print_output() {
    let output = run_and_capture(
        r#"
        print("hello");
        print(123);
    "#,
    );
    assert_eq!(output, "hello\n123\n");
}

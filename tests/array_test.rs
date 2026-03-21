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
fn array_create_read_write_and_len() {
    let output = run_and_capture(
        r#"
        let arr = [1, 2, 3];
        print(arr[0]);
        arr[1] = 10;
        print(arr[1]);
        print(len(arr));
    "#,
    );

    assert_eq!(output, "1\n10\n3\n");
}

#[test]
fn array_push_and_pop() {
    let output = run_and_capture(
        r#"
        let arr = [1];
        push(arr, 2);
        push(arr, 3);
        print(len(arr));
        print(pop(arr));
        print(len(arr));
    "#,
    );

    assert_eq!(output, "3\n3\n2\n");
}

#[test]
fn nested_array_access() {
    let output = run_and_capture(
        r#"
        let arr = [[1, 2], [3, 4]];
        print(arr[1][0]);
    "#,
    );

    assert_eq!(output, "3\n");
}

#[test]
fn index_out_of_bounds_error() {
    let program = parse_source(
        r#"
        let arr = [1, 2, 3];
        print(arr[5]);
    "#,
    )
    .unwrap();

    let mut output = Vec::new();
    let mut interpreter = Interpreter::with_output(&mut output);
    let err = interpreter.interpret(&program).unwrap_err().to_string();

    assert!(err.contains("Index out of bounds: array length is 3, index is 5"));
}

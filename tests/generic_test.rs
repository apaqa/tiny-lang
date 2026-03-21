//! 泛型測試
use tiny_lang::ast::{Expr, Statement, TypeAnnotation};
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
fn parse_generic_function_and_array_type_annotation() {
    let program = parse_source(
        r#"
        fn first<T>(arr: [T]) -> T { return arr[0]; }
        let numbers: Array<int> = [1, 2, 3];
    "#,
    )
    .unwrap();

    let Statement::FnDecl {
        type_params,
        params,
        return_type,
        ..
    } = &program[0]
    else {
        panic!("expected generic function");
    };
    assert_eq!(type_params, &vec!["T".to_string()]);
    assert!(matches!(params[0].1, Some(TypeAnnotation::ArrayOf(_))));
    assert_eq!(return_type, &Some(TypeAnnotation::Named("T".into())));

    let Statement::LetDecl {
        type_annotation: Some(TypeAnnotation::Generic { name, type_params }),
        ..
    } = &program[1]
    else {
        panic!("expected generic type annotation");
    };
    assert_eq!(name, "Array");
    assert_eq!(type_params, &vec![TypeAnnotation::Int]);
}

#[test]
fn generic_function_infers_type_parameter_from_argument() {
    let output = run_ok(
        r#"
        fn first<T>(arr: [T]) -> T { return arr[0]; }
        print(first([1, 2, 3]));
    "#,
    );

    assert_eq!(output, "1\n");
}

#[test]
fn array_generic_annotation_rejects_mixed_item_types() {
    let program = parse_source(r#"let numbers: Array<int> = [1, "two", 3];"#).unwrap();
    let err = type_check(&program).unwrap_err().to_string();
    assert!(err.contains("array literal items must have the same type") || err.contains("Array<int>"));
}

#[test]
fn generic_function_call_is_represented_as_normal_call() {
    let program = parse_source(
        r#"
        fn first<T>(arr: [T]) -> T { return arr[0]; }
        let value = first([1, 2, 3]);
    "#,
    )
    .unwrap();

    let Statement::LetDecl { value, .. } = &program[1] else {
        panic!("expected let declaration");
    };
    assert!(matches!(value, Expr::FnCall { .. }));
}

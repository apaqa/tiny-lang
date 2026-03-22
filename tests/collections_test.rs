//! stdlib/collections.tiny 整合測試。

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
fn linked_list_new_creates_empty_list() {
    let output = run_ok(
        r#"
        import "stdlib/collections.tiny";
        let list: map = linked_list_new();
        print(linked_list_is_empty(list));
        print(linked_list_size(list));
    "#,
    );
    assert_eq!(output, "true\n0\n");
}

#[test]
fn linked_list_prepend_adds_elements() {
    let output = run_ok(
        r#"
        import "stdlib/collections.tiny";
        let list: map = linked_list_new();
        list = linked_list_prepend(list, 10);
        list = linked_list_prepend(list, 20);
        list = linked_list_prepend(list, 30);
        print(linked_list_size(list));
        print(linked_list_is_empty(list));
    "#,
    );
    assert_eq!(output, "3\nfalse\n");
}

#[test]
fn linked_list_to_array_returns_elements_in_order() {
    let output = run_ok(
        r#"
        import "stdlib/collections.tiny";
        let list: map = linked_list_new();
        list = linked_list_prepend(list, 1);
        list = linked_list_prepend(list, 2);
        list = linked_list_prepend(list, 3);
        let arr: array = linked_list_to_array(list);
        print(arr[0]);
        print(arr[1]);
        print(arr[2]);
    "#,
    );
    // Prepend adds to front, so order is 3, 2, 1
    assert_eq!(output, "3\n2\n1\n");
}

#[test]
fn stack_new_creates_empty_stack() {
    let output = run_ok(
        r#"
        import "stdlib/collections.tiny";
        let s: array = stack_new();
        print(stack_is_empty(s));
        print(stack_size(s));
    "#,
    );
    assert_eq!(output, "true\n0\n");
}

#[test]
fn stack_push_and_pop_work_lifo() {
    let output = run_ok(
        r#"
        import "stdlib/collections.tiny";
        let s: array = stack_new();
        s = stack_push(s, 10);
        s = stack_push(s, 20);
        s = stack_push(s, 30);
        print(stack_size(s));
        let v1: int = stack_pop(s);
        let v2: int = stack_pop(s);
        let v3: int = stack_pop(s);
        print(v1);
        print(v2);
        print(v3);
    "#,
    );
    assert_eq!(output, "3\n30\n20\n10\n");
}

#[test]
fn stack_peek_returns_top_without_removing() {
    let output = run_ok(
        r#"
        import "stdlib/collections.tiny";
        let s: array = stack_new();
        s = stack_push(s, 42);
        s = stack_push(s, 99);
        print(stack_peek(s));
        print(stack_size(s));
    "#,
    );
    assert_eq!(output, "99\n2\n");
}

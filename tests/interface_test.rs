//! Interface/Trait 測試
use tiny_lang::ast::Program;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;
use tiny_lang::typechecker::TypeChecker;

fn run_ok(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

fn run_err(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    let mut interpreter = Interpreter::with_output(&mut output);
    interpreter.interpret(&program).unwrap_err().to_string()
}

fn type_errors(source: &str) -> Vec<String> {
    let program: Program = parse_source(source).unwrap();
    let mut checker = TypeChecker::new();
    checker.check_program(&program);
    checker.errors.into_iter().map(|err| err.message).collect()
}

#[test]
fn interface_definition_parses_and_runs() {
    let output = run_ok(
        r#"
        interface Printable {
            fn to_string(self) -> str;
        }

        struct Point { x: int, y: int }

        impl Printable for Point {
            fn to_string(self) -> str {
                return str(self.x) + "," + str(self.y);
            }
        }

        let p = Point { x: 3, y: 4 };
        print(p.to_string());
    "#,
    );

    assert_eq!(output, "3,4\n");
}

#[test]
fn impl_interface_method_dispatch_works() {
    let output = run_ok(
        r#"
        interface Greeter {
            fn greet(self, prefix: str) -> str;
        }

        struct User { name: str }

        impl Greeter for User {
            fn greet(self, prefix: str) -> str {
                return prefix + self.name;
            }
        }

        let user = User { name: "tiny" };
        print(user.greet("hello "));
    "#,
    );

    assert_eq!(output, "hello tiny\n");
}

#[test]
fn interface_method_can_be_used_as_parameter_type() {
    let errors = type_errors(
        r#"
        interface Printable {
            fn to_string(self) -> str;
        }

        struct Point { x: int }

        impl Printable for Point {
            fn to_string(self) -> str {
                return str(self.x);
            }
        }

        fn show(item: Printable) {
            print(item.to_string());
        }

        show(Point { x: 7 });
    "#,
    );

    assert!(errors.is_empty(), "unexpected type errors: {errors:?}");
}

#[test]
fn impl_missing_method_reports_error() {
    let err = run_err(
        r#"
        interface Printable {
            fn to_string(self) -> str;
            fn area(self) -> int;
        }

        struct Point { x: int, y: int }

        impl Printable for Point {
            fn to_string(self) -> str {
                return str(self.x) + "," + str(self.y);
            }
        }
    "#,
    );

    assert!(err.contains("missing method 'area'"));
}

#[test]
fn typechecker_rejects_struct_without_impl_for_interface_param() {
    let errors = type_errors(
        r#"
        interface Printable {
            fn to_string(self) -> str;
        }

        struct User { name: str }

        fn show(item: Printable) {
            print(item);
        }

        show(User { name: "tiny" });
    "#,
    );

    assert!(errors.iter().any(|err| err.contains("expects Printable, got User")));
}

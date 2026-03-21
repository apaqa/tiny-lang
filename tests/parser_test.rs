//! Parser 測試，確認 AST 結構與優先順序正確。

use tiny_lang::ast::{BinaryOperator, Expr, Statement};
use tiny_lang::parse_source;

#[test]
fn parse_all_statement_kinds() {
    let source = r#"
        let x = 10;
        x = 20;
        fn add(a, b) { return a + b; }
        if x > 1 { print(x); } else { print(0); }
        while x > 0 { x = x - 1; }
        print(x);
        add(1, 2);
    "#;

    let program = parse_source(source).unwrap();

    assert!(matches!(program[0], Statement::LetDecl { .. }));
    assert!(matches!(program[1], Statement::Assignment { .. }));
    assert!(matches!(program[2], Statement::FnDecl { .. }));
    assert!(matches!(program[3], Statement::IfElse { .. }));
    assert!(matches!(program[4], Statement::While { .. }));
    assert!(matches!(program[5], Statement::Print(_)));
    assert!(matches!(program[6], Statement::ExprStatement(_)));
}

#[test]
fn parse_operator_precedence() {
    let source = "let result = 1 + 2 * 3 == 7 || false;";
    let program = parse_source(source).unwrap();

    let Statement::LetDecl { value, .. } = &program[0] else {
        panic!("expected let declaration");
    };

    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Or,
        right,
    } = value
    else {
        panic!("expected top-level or expression");
    };

    assert_eq!(**right, Expr::BoolLit(false));

    let Expr::BinaryOp {
        left: eq_left,
        op: BinaryOperator::Eq,
        right: eq_right,
    } = &**left
    else {
        panic!("expected equality before or");
    };

    assert_eq!(**eq_right, Expr::IntLit(7));

    let Expr::BinaryOp {
        left: add_left,
        op: BinaryOperator::Add,
        right: add_right,
    } = &**eq_left
    else {
        panic!("expected addition before equality");
    };

    assert_eq!(**add_left, Expr::IntLit(1));

    let Expr::BinaryOp {
        left: mul_left,
        op: BinaryOperator::Mul,
        right: mul_right,
    } = &**add_right
    else {
        panic!("expected multiplication nested under addition");
    };

    assert_eq!(**mul_left, Expr::IntLit(2));
    assert_eq!(**mul_right, Expr::IntLit(3));
}

#[test]
fn parse_parenthesized_expression() {
    let source = "let value = (1 + 2) * 3;";
    let program = parse_source(source).unwrap();

    let Statement::LetDecl { value, .. } = &program[0] else {
        panic!("expected let declaration");
    };

    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Mul,
        right,
    } = value
    else {
        panic!("expected multiplication at top");
    };

    assert_eq!(**right, Expr::IntLit(3));

    let Expr::BinaryOp {
        left: inner_left,
        op: BinaryOperator::Add,
        right: inner_right,
    } = &**left
    else {
        panic!("expected parenthesized addition on left");
    };

    assert_eq!(**inner_left, Expr::IntLit(1));
    assert_eq!(**inner_right, Expr::IntLit(2));
}

#[test]
fn parse_phase3_statements_and_expressions() {
    let source = r#"
        for x in range(0, 3) {
            if x == 1 { continue; }
            break;
        }
        let m = {"name": "Alice"};
        let f = |x| x * 2;
        try { print(m["name"]); } catch e { print(e); }
    "#;

    let program = parse_source(source).unwrap();

    assert!(matches!(program[0], Statement::ForLoop { .. }));
    assert!(matches!(program[1], Statement::LetDecl { .. }));
    assert!(matches!(program[2], Statement::LetDecl { .. }));
    assert!(matches!(program[3], Statement::TryCatch { .. }));

    let Statement::LetDecl { value, .. } = &program[1] else {
        panic!("expected map let declaration");
    };
    assert!(matches!(value, Expr::MapLit(_)));

    let Statement::LetDecl { value, .. } = &program[2] else {
        panic!("expected lambda let declaration");
    };
    assert!(matches!(value, Expr::Lambda { .. }));
}

#[test]
fn parse_type_annotations_and_import() {
    let source = r#"
        import "examples/math_lib.tiny";
        let x: int = 10;
        let arr: [int] = [1, 2, 3];
        fn add(a: int, b: int) -> int { return a + b; }
    "#;

    let program = parse_source(source).unwrap();

    assert!(matches!(program[0], Statement::Import { .. }));

    let Statement::LetDecl {
        type_annotation: Some(_),
        ..
    } = &program[1]
    else {
        panic!("expected typed let declaration");
    };

    let Statement::FnDecl {
        params,
        return_type: Some(_),
        ..
    } = &program[3]
    else {
        panic!("expected typed function declaration");
    };

    assert_eq!(params.len(), 2);
    assert!(params.iter().all(|(_, ty)| ty.is_some()));
}

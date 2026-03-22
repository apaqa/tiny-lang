//! async/await 功能測試 - Phase 13
//! 測試 async 函式宣告、await 求值、Future 傳遞等非同步語義。

use tiny_lang::ast::Program;
use tiny_lang::interpreter::Interpreter;
use tiny_lang::parse_source;

fn run(source: &str) -> String {
    let program: Program = parse_source(source).unwrap();
    let mut output = Vec::new();
    {
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.interpret(&program).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn async_fn_definition_and_call_returns_future() {
    // 中文註解：async 函式被呼叫後回傳 Future，尚未執行本體。
    let output = run(r#"
        async fn greet() -> str { return "hello"; }
        let f = greet();
        print(f);
    "#);
    assert_eq!(output, "<future>\n");
}

#[test]
fn await_evaluates_future_and_returns_value() {
    // 中文註解：await 對 Future 求值，執行本體並回傳結果。
    let output = run(r#"
        async fn add(a: int, b: int) -> int { return a + b; }
        let result = await add(3, 4);
        print(result);
    "#);
    assert_eq!(output, "7\n");
}

#[test]
fn async_fn_returning_computed_value() {
    // 中文註解：async 函式可回傳運算結果。
    let output = run(r#"
        async fn compute(x: int) -> int { return x * x; }
        print(await compute(6));
    "#);
    assert_eq!(output, "36\n");
}

#[test]
fn nested_await_chains_async_calls() {
    // 中文註解：巢狀 await 可以鏈結多個 async 函式的結果。
    let output = run(r#"
        async fn inner(n: int) -> int { return n + 10; }
        async fn outer(n: int) -> int {
            let x = await inner(n);
            return x * 2;
        }
        print(await outer(5));
    "#);
    assert_eq!(output, "30\n");
}

#[test]
fn future_passed_as_argument_and_awaited() {
    // 中文註解：Future 可作為引數傳遞，在呼叫端 await。
    let output = run(r#"
        async fn double(n: int) -> int { return n * 2; }
        fn resolve(f) { return await f; }
        print(resolve(double(7)));
    "#);
    assert_eq!(output, "14\n");
}

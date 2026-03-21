# tiny-lang

tiny-lang 是用 Rust 實作的小型語言實驗專案，目前同時提供：

- tree-walking interpreter
- bytecode compiler + virtual machine
- formatter (`cargo run -- --fmt`)
- basic Language Server Protocol server (`cargo run -- --lsp`)

## 範例

```tiny
struct Point {
    x: int,
    y: int,
}

fn Point.distance(other: Point) -> int {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    return dx * dx + dy * dy;
}

let p = Point { x: 10, y: 20 };
let q = Point { x: 13, y: 24 };

print(p.distance(q));
```

## 功能

- 變數宣告、指定與基本運算
- `if/else`、`while`、`for in`
- `break`、`continue`、`return`
- 函式、閉包、遞迴
- array、map、struct、enum、match
- 基本型別標註與 type checker
- VM 模式、import、try/catch
- formatter：固定 4 空格縮排與穩定輸出
- LSP：parser / type checker diagnostics

## 使用方式

- interpreter REPL：`cargo run`
- VM REPL：`cargo run -- --vm`
- 執行檔案：`cargo run -- examples\showcase.tiny`
- 用 VM 執行檔案：`cargo run -- --vm examples\showcase.tiny`
- 顯示 bytecode：`cargo run -- --disasm examples\showcase.tiny`
- 格式化檔案：`cargo run -- --fmt examples\unformatted.tiny`
- 啟動 LSP：`cargo run -- --lsp`
- 執行測試：`cargo test`

## Playground

- UI 骨架在 [playground/index.html](C:/Users/阮/tiny-lang/playground/index.html)
- 目前提供深色主題、簡單 regex 高亮與 mock 輸出
- 真正執行 tiny-lang 仍需在本地使用 `cargo run`

## Examples

- `examples/hello.tiny`
- `examples/fibonacci.tiny`
- `examples/math_lib.tiny`
- `examples/import_demo.tiny`
- `examples/showcase.tiny`
- `examples/struct_demo.tiny`
- `examples/pattern_match.tiny`
- `examples/benchmark.tiny`
- `examples/unformatted.tiny`

## 測試

目前共有 **152** 個測試，涵蓋 lexer、parser、interpreter、compiler、VM、type checker、struct、method、match、GC、closure、enum、VM parity、VM import / try-catch 與 formatter。

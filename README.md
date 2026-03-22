# tiny-lang

tiny-lang ?�用 Rust 實�??��??��?言實�?專�?，目?��??��?供�?

- tree-walking interpreter
- bytecode compiler + virtual machine
- formatter (`cargo run -- --fmt`)
- basic Language Server Protocol server (`cargo run -- --lsp`)

## 範�?

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

## ?�能

- 變數�???��?定�??�本?��?
- `if/else`?�`while`?�`for in`
- `break`?�`continue`?�`return`
- ?��??��??�、�?�?- array?�map?�struct?�enum?�match
- ?�本?�別標註??type checker
- VM 模�??�import?�try/catch
- formatter：固�?4 空格縮�??�穩定輸??- LSP：parser / type checker diagnostics

## 使用?��?

- interpreter REPL：`cargo run`
- VM REPL：`cargo run -- --vm`
- ?��?檔�?：`cargo run -- examples\showcase.tiny`
- ??VM ?��?檔�?：`cargo run -- --vm examples\showcase.tiny`
- 顯示 bytecode：`cargo run -- --disasm examples\showcase.tiny`
- ?��??��?案�?`cargo run -- --fmt examples\unformatted.tiny`
- ?��? LSP：`cargo run -- --lsp`
- ?��?測試：`cargo test`

## Playground

- UI 骨架??[playground/index.html](C:/Users/??tiny-lang/playground/index.html)
- ?��??��?深色主�??�簡??regex 高亮??mock 輸出
- ?�正?��? tiny-lang 仍�??�本?�使??`cargo run`

## Examples

- `examples/hello.tiny`
- `examples/fibonacci.tiny`
- `examples/math_lib.tiny`
- `examples/import_demo.tiny`
- `examples/showcase.tiny`
- `examples/struct_demo.tiny`
- `examples/iterator_demo.tiny`
- `examples/pattern_match.tiny`
- `examples/benchmark.tiny`
- `examples/unformatted.tiny`

## 測試

?��??��? **173**?�測試�?涵�? lexer?�parser?�interpreter?�compiler?�VM?�type checker?�struct?�method?�match?�GC?�closure?�enum?�VM parity?�VM import / try-catch ??formatter??

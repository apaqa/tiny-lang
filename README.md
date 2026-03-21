# tiny-lang

tiny-lang 是用 Rust 實作的教學型語言，現在同時提供兩條執行路徑：

- tree-walking interpreter
- bytecode compiler + virtual machine

## 範例

```tiny
struct Point { x: int, y: int }

fn Point.distance(other: Point) -> int {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    return dx * dx + dy * dy;
}

let p = Point { x: 10, y: 20 };
let q = Point { x: 13, y: 24 };

print(p.distance(q));

match p.x {
    10 => { print("x is ten"); }
    _ => { print("other"); }
}
```

## 功能

- 變數宣告、指定與運算式求值
- `if/else`、`while`、`for in`
- `break`、`continue`、`return`
- 函式與遞迴
- 陣列與 map
- 型別註記：`int`、`str`、`bool`、`[type]`、`{type}`、struct 名稱、`any`
- `struct` 定義、建立、欄位讀取與欄位修改
- struct method 與 `self`
- `match` pattern matching：整數、字串、布林、識別字綁定、`_`
- bytecode compiler、disassembler 與 VM
- tree interpreter 保留作為 fallback 與參考實作

## 內建函式

- interpreter：`len`、`push`、`pop`、`str`、`int`、`type_of`、`typeof`、`input`、`range`、`keys`、`values`、`abs`、`max`、`min`、`pow`、`split`、`join`、`trim`、`upper`、`lower`、`contains`、`replace`、`sort`、`reverse`、`map`、`filter`、`reduce`、`find`、`assert`
- VM：`len`、`push`、`pop`、`str`、`int`、`type_of`、`typeof`、`range`

## 執行

- tree interpreter REPL：`cargo run`
- VM REPL：`cargo run -- --vm`
- 執行檔案（interpreter）：`cargo run -- examples\\showcase.tiny`
- 執行檔案（VM）：`cargo run -- --vm examples\\showcase.tiny`
- 顯示 bytecode：`cargo run -- --disasm examples\\showcase.tiny`
- 顯示 bytecode 並以 VM 執行：`cargo run -- --disasm --vm examples\\showcase.tiny`
- 測試：`cargo test`

## Benchmark

- benchmark 範例：[examples/benchmark.tiny](C:/Users/阮/tiny-lang/examples/benchmark.tiny)
- 建議比較：
  - interpreter：`cargo run -- examples\\benchmark.tiny`
  - VM：`cargo run -- --vm examples\\benchmark.tiny`

## 速度對比

以下數字會依機器與 build profile 改變。以目前工作區的 `target/debug/tiny-lang.exe` 實測：

| 模式 | benchmark.tiny |
| --- | --- |
| tree interpreter | `fib(30)` 在目前 debug build 下觸發 stack overflow |
| bytecode VM | 約 `3196.8 ms` |

## examples

- `examples/hello.tiny`
- `examples/fibonacci.tiny`
- `examples/math_lib.tiny`
- `examples/import_demo.tiny`
- `examples/showcase.tiny`
- `examples/struct_demo.tiny`
- `examples/pattern_match.tiny`
- `examples/benchmark.tiny`

## 測試數量

目前共有 **83** 個測試，涵蓋 lexer、parser、interpreter、compiler、VM、型別、struct、method、match 與其他標準功能。

# tiny-lang

tiny-lang 是用 Rust 實作的教學型直譯語言，包含 `lexer`、`parser`、AST、tree-walking interpreter，以及 Phase 5 新增的 bytecode compiler 基礎骨架。

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
- 函式、lambda、closure
- 陣列與 map
- 型別註記：`int`、`str`、`bool`、`[type]`、`{type}`、struct 名稱、`any`
- `try/catch`
- `import "file.tiny";`
- `struct` 定義、建立、欄位讀取與欄位修改
- struct method 與 `self`
- `match` pattern matching：整數、字串、布林、識別字綁定、`_`
- bytecode compiler foundation：目前可編譯基礎表達式與簡單 statement，保留後續 VM 擴充空間

## 內建函式

- 集合：`len`、`push`、`pop`、`range`、`keys`、`values`
- 轉型與輸入：`str`、`int`、`type_of`、`typeof`、`input`
- 數值：`abs`、`max`、`min`、`pow`
- 字串：`split`、`join`、`trim`、`upper`、`lower`、`contains`、`replace`
- 陣列高階函式：`sort`、`reverse`、`map`、`filter`、`reduce`、`find`
- 驗證：`assert`

## 執行

- REPL：`cargo run`
- 執行範例：`cargo run -- examples\\showcase.tiny`
- 測試：`cargo test`

## examples

- `examples/hello.tiny`
- `examples/fibonacci.tiny`
- `examples/math_lib.tiny`
- `examples/import_demo.tiny`
- `examples/showcase.tiny`
- `examples/struct_demo.tiny`
- `examples/pattern_match.tiny`

## 測試數量

目前共有 **71** 個測試，涵蓋 lexer、parser、interpreter、型別、import、closure、struct、method、match 與其他標準功能。

# tiny-lang

tiny-lang 是一個用 Rust 實作的教學型直譯語言，目標是用小而清楚的程式碼，展示 lexer、parser、AST、直譯器、型別檢查、模組系統和標準庫如何串在一起。

## 語法範例

```tiny
import "examples/math_lib.tiny";

let nums: [int] = range(0, 5);
let doubled = map(nums, |x| x * 2);

fn add(a: int, b: int) -> int {
    return a + b;
}

for n in doubled {
    if n == 4 {
        continue;
    }
    print(add(n, square(2)));
}

try {
    assert(len(doubled) == 5, "length mismatch");
} catch e {
    print(e);
}
```

## 支援功能

- 變數宣告、指定、算術與邏輯運算
- `if/else`、`while`、`for in`
- `break`、`continue`、`return`
- 字串、陣列、Map
- 函式、匿名函式、箭頭閉包
- 可選型別標註：`int`、`str`、`bool`、`[type]`、`{type}`、`any`
- `try/catch` 執行期錯誤處理
- `import "file.tiny";` 模組載入與重複匯入去重

## 內建函式

- 基本：`len`、`push`、`pop`、`str`、`int`、`type_of`、`typeof`、`input`、`range`、`keys`、`values`
- 數學：`abs`、`max`、`min`、`pow`
- 字串：`split`、`join`、`trim`、`upper`、`lower`、`contains`、`replace`
- 陣列：`sort`、`reverse`、`map`、`filter`、`reduce`、`find`
- 工具：`assert`

## 執行方式

- REPL：`cargo run`
- 執行檔案：`cargo run -- examples\showcase.tiny`
- 測試：`cargo test`

## 範例程式

- `examples/hello.tiny`：最小 hello world
- `examples/fibonacci.tiny`：函式與迴圈
- `examples/math_lib.tiny`：可被 import 的模組
- `examples/import_demo.tiny`：匯入示範
- `examples/showcase.tiny`：語言功能總覽

## 測試數量

目前共有 **56** 個測試，涵蓋 lexer、parser、interpreter、型別標註、import、標準庫、closure、Map、for 迴圈與錯誤處理。

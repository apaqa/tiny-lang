# tiny-lang

A toy programming language implemented in Rust — tree-walking interpreter, bytecode compiler, GC, LSP, and formatter all in one codebase.

![Tests](https://img.shields.io/badge/tests-183%20passing-brightgreen)
![Language](https://img.shields.io/badge/written%20in-Rust-orange)

## Overview

tiny-lang is a statically-typed scripting language built as a learning project. It features a hand-written lexer and recursive-descent parser, a tree-walking interpreter for rapid execution, and a bytecode compiler + register-based VM for a second execution path. A garbage collector manages heap objects (arrays, maps, structs, closures), and a basic LSP server provides editor diagnostics.

Both execution paths share the same AST and pass the same test suite, ensuring behavioral parity.

## Quick Start

```sh
# REPL (interpreter)
cargo run

# REPL (VM)
cargo run -- --vm

# Run a file
cargo run -- examples/showcase.tiny

# Run a file with the VM
cargo run -- --vm examples/showcase.tiny

# Show VM bytecode disassembly
cargo run -- --disasm examples/showcase.tiny

# Format a file (prints to stdout)
cargo run -- --fmt examples/unformatted.tiny

# Start LSP server (stdio)
cargo run -- --lsp

# Run tests
cargo test
```

## Language Tour

### Variables and basic types

```tiny
let x: int = 42;
let name: str = "Alice";
let flag: bool = true;
let nothing = null;
```

### Functions and closures

```tiny
fn add(a: int, b: int) -> int {
    return a + b;
}

let double = |x| { return x * 2; };
print(double(add(3, 4)));  // 14
```

### Structs and methods

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
print(p.distance(q));  // 25
```

### Enums and match

```tiny
enum Shape {
    Circle { radius: int },
    Rect   { w: int, h: int },
}

let s = Shape::Circle { radius: 5 };
match s {
    Shape::Circle { radius } => { print(radius); }
    Shape::Rect   { w, h }   => { print(w * h);  }
}
```

### Interfaces (traits)

```tiny
interface Describable {
    fn describe() -> str;
}

struct Dog { name: str }
impl Dog: Describable {
    fn Dog.describe() -> str { return self.name; }
}
```

### Generics

```tiny
fn identity<T>(value: T) -> T {
    return value;
}
```

### Result and Option

```tiny
let ok:   Result<int, str> = Result::Ok(10);
let none: Option<int>      = Option::None;

match ok {
    Result::Ok(v)  => { print(v); }
    Result::Err(e) => { print(e); }
}
```

### async / await

```tiny
async fn fetch(url: str) -> str {
    return "response";
}

let future = fetch("https://example.com");
let result = await future;
print(result);
```

### for / in with iterators

```tiny
for item in [1, 2, 3] {
    print(item);
}

// Custom iterator — implement next() returning Option<T>
struct Counter { value: int }
fn Counter.next() -> Option<int> {
    if self.value > 3 { return Option::None; }
    let v = self.value;
    self.value = self.value + 1;
    return Option::Some(v);
}

let c = Counter { value: 1 };
for n in c { print(n); }  // 1 2 3
```

### Namespace imports

```tiny
import "stdlib/math_ext.tiny" as math;
import "stdlib/collections.tiny" as coll;

print(math.gcd(12, 8));       // 4
print(math.fibonacci(10));    // 55

let s = coll.stack_new();
s = coll.stack_push(s, 42);
print(coll.stack_pop(s));     // 42
```

## Standard Library

### Built-in functions

| Function | Description |
|---|---|
| `len(x)` | Length of array/string/map |
| `push(arr, v)` / `pop(arr)` | Array mutation |
| `str(x)` / `int(x)` | Type coercion |
| `type_of(x)` | Runtime type name |
| `range(start, end)` | Integer range array |
| `keys(m)` / `values(m)` | Map keys/values |
| `abs(n)` / `pow(b,e)` / `max(a,b)` / `min(a,b)` | Math |
| `split(s,sep)` / `join(arr,sep)` / `trim(s)` | String |
| `upper(s)` / `lower(s)` / `contains(s,sub)` / `replace(s,a,b)` | String |
| `sort(arr)` / `reverse(arr)` | Array ordering |
| `map(arr,fn)` / `filter(arr,fn)` / `reduce(arr,fn,init)` | Functional |
| `assert(cond)` | Testing |

### stdlib/math_ext.tiny

`gcd`, `lcm`, `factorial`, `fibonacci`, `is_prime`, `abs_val`

### stdlib/collections.tiny

`linked_list_new`, `linked_list_prepend`, `linked_list_to_array`, `linked_list_size` — linked list built on maps.

`stack_new`, `stack_push`, `stack_pop`, `stack_peek`, `stack_size` — LIFO stack built on arrays.

## Type System

- Optional type annotations on variables, parameters, and return types
- Static type checker run before interpretation (warns on type mismatches)
- Generic functions and structs: `fn id<T>(x: T) -> T`
- Built-in `Option<T>` and `Result<T, E>` enums with match destructuring
- Interface declarations + `impl Struct: Interface` blocks

## Architecture

```
Source code
    │
    ▼
┌─────────┐
│  Lexer  │  token.rs / lexer.rs  — produces SpannedToken stream
└────┬────┘
     │
     ▼
┌──────────┐
│  Parser  │  parser.rs  — recursive descent → AST (ast.rs)
└────┬─────┘
     │
     ├──────────────────────────┐
     ▼                          ▼
┌─────────────┐         ┌────────────┐
│ TypeChecker │         │  Formatter │  formatter.rs
│ typechecker │         └────────────┘
└──────┬──────┘
       │
       ├─────────────────────────────┐
       ▼                             ▼
┌─────────────┐               ┌──────────────┐
│ Interpreter │               │   Compiler   │  compiler.rs
│ interpreter │               └──────┬───────┘
└─────────────┘                      │
   tree-walk                         ▼
   execution                  ┌──────────┐
   environment.rs             │    VM    │  vm.rs
                              └────┬─────┘
                                   │
                                   ▼
                            ┌────────────┐
                            │     GC     │  gc.rs — mark-and-sweep
                            └────────────┘
```

## Testing

**183 tests** covering: lexer, parser, interpreter, compiler, VM, type checker, structs, methods, closures, enums, match, generics, GC, async/await, iterators, Result/Option, try/catch, formatter, VM parity, import, stdlib collections, namespace import.

```sh
cargo test                          # all tests
cargo test --test interpreter_test  # single suite
cargo test async                    # filter by name
```

## Project Structure

```
tiny-lang/
├── src/
│   ├── main.rs          # CLI entry point (REPL, file runner, flags)
│   ├── lib.rs           # Public API (parse_source, type_check)
│   ├── token.rs         # Token enum
│   ├── lexer.rs         # Tokenizer
│   ├── ast.rs           # AST node definitions
│   ├── parser.rs        # Recursive-descent parser
│   ├── typechecker.rs   # Static type checker
│   ├── environment.rs   # Runtime values, environments, GC-managed types
│   ├── interpreter.rs   # Tree-walking interpreter
│   ├── compiler.rs      # Bytecode compiler (AST → OpCode)
│   ├── vm.rs            # Stack-based virtual machine
│   ├── gc.rs            # Mark-and-sweep garbage collector
│   ├── formatter.rs     # Code formatter
│   ├── error.rs         # Error types
│   └── lsp.rs           # LSP server (diagnostics only)
├── stdlib/
│   ├── collections.tiny # LinkedList + Stack
│   └── math_ext.tiny    # gcd, lcm, factorial, fibonacci, is_prime
├── tests/               # 33 integration test suites
├── examples/            # Sample programs
└── playground/          # Browser UI skeleton (index.html)
```

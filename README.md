---
title: "Fiat — A Homoiconic Lisp Interpreter in Rust"
tags: [fiat, lisp, interpreter, rust, orpheus, game-engine, scripting, cons-list, tree-walking]
domain: programming-languages
status: in-development
---

# Fiat

Fiat is a homoiconic Lisp dialect with its core implemented in Rust. It targets two use cases: general-purpose scripting (replacing bash, Python, Perl) and embeddable scripting for the Orpheus game engine (occupying the same role as Lua). The total primitive count is eight special forms plus arithmetic and set operations — 21 operations from which the entire language is built.

Fiat's design priorities, in order, are: token efficiency for LLM generation, readability for human inspection, and round-trip stability for the cycle of generation → reading → editing → re-generation.


## Building and Running

Fiat builds to a single self-contained binary — the standard library and prelude are embedded at compile time, so there are no runtime files to ship alongside it.

```sh
cargo build --release        # produces target/release/fiat
```

To use `fiat` as a command-line tool anywhere, copy the binary onto your `PATH`. On macOS, `/bin` is protected by System Integrity Protection and is not writable; use `/usr/local/bin` (Intel) or `/opt/homebrew/bin` (Apple Silicon) instead — both are on `PATH` by default:

```sh
cp target/release/fiat /usr/local/bin/    # or /opt/homebrew/bin on Apple Silicon
```

On Linux, `~/.local/bin` or `/usr/local/bin` work the same way. Alternatively, `cargo install --path .` places `fiat` in `~/.cargo/bin`.

### Usage

```sh
fiat                 # start the REPL (prelude loaded)
fiat lux             # start the REPL with the Lux standard library imported
fiat path/to.fiat    # run a Fiat source file, printing the final value
fiat --no-prelude …  # use a bare environment (Levels 0 and 6 run this way)
fiat --help          # full usage
fiat --version       # version
```

The REPL reads a form and evaluates it on Enter; multi-line forms continue (with a `...` prompt) until the parentheses balance. Ctrl-D exits, and command history persists in `~/.fiat_history`. `fiat lux` is the quickest way to explore the language interactively — `String/*`, `Map/*`, `Math/*`, and the rest of Lux are available immediately, without typing `(fiat Lux)` first.

### The REPL

The REPL is a read-eval-print loop built on the same pipeline the file runner uses: each submitted form is desugared, evaluated, and its result printed. It lives in `src/main.rs` and is started by `fiat` (prelude only) or `fiat lux` (prelude plus the Lux standard library pre-imported).

**Line editing and history.** Input is handled by the [`rustyline`](https://crates.io/crates/rustyline) crate — a pure-Rust readline implementation (no GNU readline C dependency). It provides Emacs-style line editing (cursor movement, kill/yank), arrow-key recall of previous entries, and reverse search. History is loaded at startup and saved on exit to `~/.fiat_history` (resolved from `$HOME`), so it carries across sessions. `rustyline` is the only dependency the REPL adds beyond the interpreter core; the library and benchmark code do not depend on it.

**Multi-line input.** After each line, the REPL attempts to parse the accumulated buffer. If the reader reports the input is *incomplete* — an unterminated list, vector, map, set, or string — the prompt switches to `  ... ` and another line is read, repeating until the form parses. This is how a definition like

```lisp
fiat> (fiat square (x)
  ...   (* x x))
<function square>
fiat> (square 9)
81
```

is entered across several lines. The distinction between "incomplete" and "malformed" input is made by inspecting the reader error: messages beginning with `unterminated` (or `unexpected end of input`) mean *keep reading*, while any other parse error (for example a stray `)`) is reported immediately and the buffer is discarded.

**Evaluation semantics.** A submitted buffer may contain more than one form; each is evaluated in order and every result is printed, against a single persistent environment, so bindings made earlier in the session remain visible later. Because a named `(fiat name …)` declaration returns the function value, defining a function echoes `<function name>` — confirming the binding took. Evaluation errors are printed (`error: …`) without ending the session.

**Signals.** Ctrl-C abandons the current (possibly multi-line) input and returns to a fresh `fiat> ` prompt; Ctrl-D at the prompt saves history and exits. Both behaviours come from `rustyline`'s `Interrupted` and `Eof` signals.

**Environments.** `fiat` loads the self-hosted prelude (`map`, `filter`, `fold`, `sort`, …); `fiat lux` additionally evaluates `(fiat Lux)` once at startup so namespaced standard-library functions resolve without an explicit import; `fiat --no-prelude` starts from a bare environment for experimenting with just the primitive kernel. In every case Firmamentum is registered (standalone scripting mode), so `(fiat Firmamentum)` and the `Fs/*` capabilities are available.


## Interpreter Architecture: Bootstrap and Final Pipeline

Fiat's final architecture processes source through three stages: reading, macro expansion, and evaluation. However, the initial interpreter uses a smaller bootstrap pipeline so the evaluator can come online before the full hygienic macro system exists.

**Bootstrap pipeline:** source text → reader → minimal desugar pass → evaluator.

**Final pipeline:** source text → reader → hygienic macro expander → evaluator.

The bootstrap desugar pass handles only the small set of syntactic forms needed by the early benchmark levels: `let` (Level 1) and the threading operators `->`, `->>`, and `name->` (Level 3). Quote shorthand (`'x` → `(behold x)`) is handled directly in the reader. The full macro expander — syntax objects, hygienic renaming, user-defined macros, and scope-aware expansion — is implemented later, after the evaluator, closures, collections, and module loading are already working and validated through the benchmark suite.

**Reader** — converts source text into `Value` instances (s-expressions). The initial reader (v0) handles lists, atoms, keywords, quote shorthand, and comments. A later extension (v1) adds vector, map, and set literal syntax when those runtime types exist.

**Evaluator** — implements the eight primitive special forms, function application, and arithmetic. Atoms that are numbers, strings, keywords, booleans, or nil evaluate to themselves. Symbols look up their binding in the current environment. Lists evaluate by treating their first element as an operator: if it names a special form (`behold`, `choose`, `fiat`, etc.), that form's evaluation rules apply; otherwise the first element is evaluated as a function and applied to the evaluated arguments.


## Value System: The Universal Representation

Every value in Fiat is a variant of a single Rust enum. Heap-allocated structures such as strings, lists, vectors, maps, sets, and functions are stored behind `Rc` pointers — no garbage collector, no `unsafe`, no interaction with the Rust borrow checker. Exact enum size is an implementation detail and should be measured with `std::mem::size_of::<Value>()` once the enum stabilizes.

```rust
use std::rc::Rc;

enum Value {
    Nil,                                    // also serves as the empty list ()
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<str>),
    Symbol(InternedSymbol),
    Keyword(InternedSymbol),                // :ok, :err, :player, :dept, etc.
    List(Rc<Cons>),                         // non-empty cons list
    Vector(Rc<PersistentVector<Value>>),     // persistent indexed sequence (added at Level 2)
    Map(Rc<PersistentMap<Value, Value>>),    // persistent key-value (added at Level 2)
    Set(Rc<PersistentSet<Value>>),           // persistent unordered (added at Level 2)
    Function(Rc<Function>),
}

struct Cons {
    head: Value,
    tail: Value,    // must be Nil or List(Rc<Cons>)
}
```

Values divide into atoms (indivisible: nil, booleans, integers, floats, strings, symbols, keywords) and collections (compound: lists, vectors, maps, sets). All data is immutable by default. Keywords are colon-prefixed atoms (`:ok`, `:err`, `:name`, `:dept`, `:player`, `:has-quest`) used throughout the benchmark suite as map keys, tags, and symbolic constants — they must be present from the first milestone. The persistent collection types (vectors, maps, sets) are HAMT-backed with structural sharing and are added when Level 2 requires them. Lists use a cons-cell chain with different trade-offs, explained in the next section.


## List Representation: Cons Cells and the bind/first/rest Algebra

Fiat lists are cons-cell chains, not array-backed vectors. This representation is dictated by the language's list algebra: `bind` prepends a value, `first` takes the head, and `rest` returns the tail. A list like `(a b c)` is internally `(bind 'a (bind 'b (bind 'c '())))` — a chain of cons cells terminating in `Nil`. The identity law holds: for any non-empty list `x`, `(bind (first x) (rest x))` produces `x`.

With cons cells, all three primitive list operations are O(1). `first` reads the `head` field. `rest` reads the `tail` field (which is already a complete list via `Rc` sharing). `bind` allocates a single new `Cons` and points its `tail` at the existing list — but only after validating that the tail is `Nil` or `List`, rejecting non-list tails to preserve the cons-chain invariant.

```rust
fn first(list: &Value) -> Result<Value, Error> {
    match list {
        Value::List(cell) => Ok(cell.head.clone()),
        Value::Nil => Err(Error::first_on_empty_list()),
        _ => Err(Error::type_error("list", list.type_name())),
    }
}

fn rest(list: &Value) -> Result<Value, Error> {
    match list {
        Value::List(cell) => Ok(cell.tail.clone()),
        Value::Nil => Ok(Value::Nil),
        _ => Err(Error::type_error("list", list.type_name())),
    }
}

fn bind(val: Value, list: Value) -> Result<Value, Error> {
    match &list {
        Value::Nil | Value::List(_) => {
            Ok(Value::List(Rc::new(Cons { head: val, tail: list })))
        }
        other => Err(Error::type_error("list", other.type_name())),
    }
}
```

The trade-off is that indexed access into a cons list is O(n) — getting the 500th element requires walking 500 cells. This is exactly why Fiat has a separate vector type (`[]` syntax, backed by a persistent HAMT vector): lists are for recursive decomposition and code-as-data; vectors are for indexed ordered collections.

An array-backed representation like `Rc<Vec<Value>>` would make `bind` and `rest` O(n) operations (requiring allocation and copying), which breaks the cost model that every benchmark program assumes. Level 0's recursive `reverse`, `zip`, `append`, and `flatten` all call `first`, `rest`, and `bind` on every recursive step — with a `Vec` backing, these would become O(n²) despite looking linear in the source code.

The reader bridges the gap between left-to-right parsing and right-to-left list construction by collecting elements into a temporary `Vec`, then folding into a cons chain:

```rust
fn list_from_vec(items: Vec<Value>) -> Value {
    items
        .into_iter()
        .rev()
        .fold(Value::Nil, |tail, head| {
            Value::List(Rc::new(Cons { head, tail }))
        })
}
```

The empty list `()` is represented as `Value::Nil`, which means `Nil` and the empty list are identical — `(atom? ())` returns `true`. `first` on `Nil` returns a runtime error rather than silently returning `Nil`, because catching empty-list decomposition errors early prevents subtle bugs downstream.


## Lexical Scoping and Environment Chain

Variable references resolve lexically — a variable refers to the binding in the closest enclosing scope where it was defined, determined by the textual structure of the program, not the runtime call stack. Environments form a parent-linked chain: each environment holds a map of bindings wrapped in `RefCell` for interior mutability, and an optional `Rc` pointer to its parent.

```rust
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct Env {
    bindings: RefCell<HashMap<InternedSymbol, Value>>,
    parent: Option<Rc<Env>>,
}
```

The bindings map uses `RefCell` so named `fiat` declarations can support recursion. A named function must capture an environment in which its own name is bound. With plain `Rc<Env>` and an immutable `HashMap`, you cannot create the function, insert it into the environment, and have the closure see itself. `RefCell` allows the environment to be mutated after the closure captures it: the function is allocated, inserted into the enclosing environment under its own name, and the closure's captured reference to that environment now resolves the self-reference during recursive calls.

Closures capture a reference to the environment that was active when they were created. This reference is fixed at creation time — the closure always sees the same bindings regardless of where it is later called. Multiple closures can share the same parent environment via `Rc`.


## The Eight Primitive Special Forms

Fiat has eight primitive operators implemented in Rust, from which everything else is derived. The following table lists each primitive and its role, because every benchmark level exercises subsets of this primitive set.

| Primitive | Purpose |
|-----------|---------|
| `behold`  | Arrest evaluation — return an expression as data, unevaluated. `'x` is shorthand. |
| `first`   | Return the head of a cons cell. O(1). Error on `Nil`. |
| `rest`    | Return the tail of a cons cell. O(1). Returns `Nil` on `Nil`. |
| `bind`    | Allocate a new cons cell prepending a value to a list. O(1). Rejects non-list tails. |
| `atom?`   | Test whether a value is an atom (not a collection). `Nil` is atomic. Returns boolean. |
| `is?`     | Atom pointer identity test — see below for heap-type semantics. Error on collections. |
| `choose`  | Multi-armed conditional. Evaluates test-expression pairs in order, returns the result of the first true test. |
| `fiat`    | Declare functions (named or anonymous) and import modules. |

`is?` is defined only for atoms and uses pointer identity. For inline types (integers, floats, booleans, nil), pointer identity is equivalent to value equality. For interned types (symbols, keywords), interning guarantees that equal names share the same reference, so `is?` is reliable. For strings (`Rc<str>`), `is?` compares the `Rc` pointer — two separately constructed strings with identical content are *not* `is?`-equal. String value comparison uses `String` module functions, not `is?`. `is?` returns an error when called on collections. Structural equality for lists, vectors, maps, and sets is a separate concern, derived in the standard library rather than part of the primitive kernel.


## fiat Declaration Semantics

The `fiat` form serves two roles, disambiguated by argument shape. This section defines the dispatch rules and return-value semantics so the evaluator handles both roles correctly from Level 0 onward.

**Shape dispatch:** `(fiat CapitalName)` with one capitalized symbol is a module import. `(fiat () (params...) body...)` with `()` in the name slot creates an anonymous function. `(fiat name (params...) body...)` with a lowercase symbol creates a named function and binds it in the current environment. Any other shape is a malformed `fiat` error.

**Multi-form bodies:** a `fiat` body may contain one or more forms. When multiple forms are present, they are evaluated in order in the function's local environment, and the value of the final form is returned. This allows local helper definitions without requiring a separate `do` or `begin` form:

```lisp
(fiat outer (x)
  (fiat helper (y) (+ y 1))
  (helper x))
```

Local named `fiat` declarations are visible to later forms in the same body.

**Return value:** a named `fiat` declaration binds the function in the current environment and returns the function value. This makes declarations usable in expression position and inspectable in the REPL.

**Module import:** during the bootstrap phase, `(fiat CapitalName)` can return an "unimplemented module import" error until Module System v0 is reached. Function declaration must work for Level 0.


## Program Evaluation Semantics

A Fiat source file is a sequence of top-level forms. The interpreter reads all forms, applies desugaring or macro expansion, then evaluates them in order in the same top-level environment. Definitions created by earlier forms remain visible to later forms. The value of the program is the value of the final form.

```text
program := form*
```

This rule is used by the REPL (evaluate one form at a time, printing each result), the file runner (evaluate all forms, optionally print the final result), and the benchmark harness (evaluate the file and assert the final result). The file runner and benchmark harness both need to process files containing multiple `fiat` declarations followed by test expressions.


## Interpreter Runtime Errors vs Fiat Result Values

The Rust interpreter returns `Result<Value, Error>` internally for all evaluation. Type errors, unbound symbols, invalid special-form shapes, and illegal primitive calls are interpreter runtime errors — they abort evaluation of the current expression and propagate up to the top-level handler.

Fiat-level result values such as `{:ok value}` and `{:err reason}` are ordinary Fiat data used by user programs (Level 4). They do not replace interpreter errors during the bootstrap phase. The two systems are independent: interpreter errors are Rust-side control flow; Fiat result values are data that flows through normal evaluation.

The following interpreter runtime errors should be defined from the start: unbound symbol, attempting to call a non-function, malformed special form (wrong argument count or shape), `first` on `Nil`, `first` or `rest` on non-list, `bind` with non-list tail, `is?` called on collections, arithmetic type mismatch (non-numeric arguments), and division by zero.


## Benchmark Support Builtins and Prelude Functions

The eight primitive special forms are the semantic kernel, but the benchmark suite requires additional built-in functions and prelude definitions beyond those eight. This section assigns each needed operation to its implementation layer so nothing is missing when a benchmark level is attempted.

**Numeric primitives** are implemented in Rust and registered in the global environment: `+`, `-`, `*`, `/`, `%`, `>`, `<`, `=`. These eight operations are part of the Rust kernel — `%` is included because integer remainder, like division, is a hardware-level operation that cannot be derived from the other arithmetic primitives.

**Derived numeric functions** are defined in the prelude using the primitives above: `>=`, `<=`, `max`, `min`. These are ordinary Fiat functions, not Rust builtins — `>=` is `(not (< a b))`, `<=` is `(not (> a b))`, `max` and `min` use `choose`. All twelve operations are available at runtime in every environment, but the layering matters: the first eight are part of the kernel's operation count (21 total: 8 special forms + 8 arithmetic + 5 set operations); the last four are prelude definitions like `map` and `filter`.

**Boolean and logical functions** are desugared to `choose` expressions during the bootstrap desugar pass: `not` is a prelude function or builtin, while `and` and `or` require short-circuit evaluation and are desugared rather than implemented as functions (which would eagerly evaluate all arguments).

**Math module** is provided by Lux: `Math/sqrt`. Needed by Level 7 (collision detection).

**Int module** is provided by Lux: `Int/parse`, `Int/to-string`. Needed by Level 7 (entity system formatting) and Level 8 (CSV parsing).

**Float module** is provided by Lux: `Float/to-string` and related conversions as needed.

**List and prelude functions** Everything beyond the primitive kernel is either self-hosted Fiat code or host/Rust module bindings, depending on whether it needs direct access to runtime representation.


## Collection Literal Evaluation Semantics

Collection literals (`[]` vectors, `{}` maps, `#{}` sets) evaluate their elements when they appear in evaluated code. This follows Clojure's model: symbols and expressions inside an unquoted collection literal are evaluated, producing a collection of computed values. Quoted collection literals (inside `behold`) are inert data.

The reader never evaluates collection elements. It only constructs collection syntax values containing raw, unevaluated forms. Element evaluation happens in the evaluator when a collection literal is evaluated outside `behold`.

```lisp
(let ((x 5))
  [x (+ x 1) (+ x 2)])    ;; → [5 6 7], elements evaluated

(behold [x (+ x 1)])       ;; → [x (+ x 1)], elements unevaluated
```

Map literals use alternating key/value forms. The reader must reject maps with an odd number of forms:

```lisp
{:name "Alice" :dept :eng}    ;; two key-value pairs
{:x (+ 1 2) :y (* 3 4)}      ;; keys are keywords, values are evaluated
```

This decision must be made before Level 2, because the benchmark programs use variable references and function calls inside collection literals.


## Collection Iteration: Lists as the Sequence Type

The prelude functions `fold`, `map`, and `filter` are self-hosted Fiat functions built on the list primitives `first`, `rest`, `atom?`, and `bind`. They operate on lists only. This is a direct consequence of self-hosting: these functions are defined in Fiat using the eight primitives, and the list primitives are cons-cell operations that do not accept vectors.

Code that needs to iterate over a vector converts it to a list first using `Vector/to-list`. The conversion is explicit, following the same principle that makes string decomposition explicit (`as-codepoints` vs `as-graphemes` vs `as-bytes`): when switching between representations with different performance characteristics, the code should say so. Level 2 benchmark 2d, which groups vector records by key, calls `Vector/to-list` at the boundary before passing data into `group-by`.


## Module System and Capability Gating

Modules are imported with `(fiat ModuleName)` where module names are capitalized. The standard library has two tiers that control what capabilities are available in a given environment.

**Lux** — the core standard library. Pure-computation modules for built-in data types: `String`, `Map`, `Set`, `Vector`, `List`, `Int`, `Float`, `Math`. No I/O, no OS interaction. Available everywhere. Every program that uses standard library functions begins with `(fiat Lux)`.

**Firmamentum** — the scripting capability layer. OS-level bindings: `Fs` (filesystem), `Process`, `Net`, `Http`. Only available when the host registers it. Present in standalone scripting mode; absent in embedded contexts like Orpheus.

Capability gating works by module presence, not runtime permission checks. If `Firmamentum` is not registered, the syntax for filesystem access does not exist — there is no surface to accidentally expose and no permission check to bypass. The module system is split into two implementation milestones: v0 (Lux import and namespaced lookup, needed for Level 2) and v1 (host-registered capabilities and Firmamentum, needed for Level 8).


## Project Structure

The following directory layout separates the interpreter into focused modules, with integration tests mirroring the benchmark levels and standalone `.fiat` source files for each level.

```
fiat/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs              # CLI entry: REPL, file runner, `lux` subcommand
│   ├── lib.rs               # Crate root: module declarations and public API
│   ├── value.rs             # Value enum, Cons struct, Keyword, Display, PartialEq
│   ├── env.rs               # Env with RefCell<HashMap>, scope chain, lookup
│   ├── error.rs             # Runtime error type and common error constructors
│   ├── reader.rs            # Source text → Value (recursive descent, list_from_vec)
│   ├── printer.rs           # Value → source text (inverse of reader)
│   ├── desugar.rs           # Minimal desugar pass: let, threading macros
│   ├── eval.rs              # Evaluator: 8 primitives, arithmetic/set builtins, application, TCO
│   ├── prelude.rs           # Loader for the embedded self-hosted prelude
│   ├── prelude.fiat         # Self-hosted prelude: map, filter, fold, sort, etc.
│   └── modules/
│       ├── mod.rs            # Module registry, Lux loader, capability gating
│       ├── string.rs         # String module (split, trim, upcase, etc.)
│       ├── map.rs            # Map module (get, put, merge, entries, etc.)
│       ├── vector.rs         # Vector module (append, nth, to-list, etc.)
│       ├── math.rs           # Math module (sqrt, etc.)
│       ├── int.rs            # Int module (parse, to-string)
│       ├── float.rs          # Float module (to-string)
│       └── firmamentum.rs    # Scripting I/O bindings (Fs, Process, Net)
├── lib/                      # Self-hosted Lux modules (Fiat source, loaded by `fiat Lux`)
│   ├── Set.fiat              # Set/ namespace over the set primitives
│   └── List.fiat             # List/ namespace over list ops and prelude functions
├── tests/
│   ├── level0_primitives.rs
│   ├── level1_closures.rs
│   ├── level2_collections.rs
│   ├── level3_strings.rs
│   ├── level4_results.rs
│   ├── level5_state_machines.rs
│   ├── level6_combinators.rs
│   ├── level7_entities.rs
│   └── level8_scripting.rs
└── benchmarks/               # Fiat source files from the benchmark suite
    ├── level0.fiat
    ├── level1.fiat
    ├── level2.fiat
    ├── level3.fiat
    ├── level4.fiat
    ├── level5.fiat
    ├── level6.fiat
    ├── level7.fiat
    ├── level8.fiat
    └── employees.csv         # Fixture for the Level 8 file-processing benchmark
```


## Benchmark Suite: Levels and What They Validate

The benchmark suite is a sequence of nine levels (0 through 8) in ascending complexity. Each level exercises a distinct interpreter capability. A level passes when all of its programs produce the expected output. Levels are designed to be attacked in order — each level depends on capabilities validated by previous levels.

### Level 0 — Primitives Only

No imports, no standard library. Tests the evaluator, the eight primitives, arithmetic, and basic recursion. Four programs: list construction round-trip (validates `bind`/`first`/`rest`/`is?` and the identity law), list reversal (validates tail recursion and `bind` as sole constructor), zip (validates nested `bind` and parallel recursion), and flatten (validates recursive descent into sublists with `atom?` distinguishing leaves). Because every program in this level calls `first`, `rest`, and `bind` on every recursive step, this is also the first validation that the cons-cell representation delivers the expected O(1) cost per operation.

### Level 1 — Closures and Higher-Order Functions

Tests lexical scoping, closure capture, and functions as values. Requires `let` desugaring. Four programs: function composition (validates returning closures that capture arguments), currying via partial application (validates nested closure capture with lexical binding), Church booleans (validates higher-order application and closures as data), and accumulator factory (validates closures over recursive state in pure functional style — returns a proper two-element list `(value next-counter)`, not an improper pair, because `bind` rejects non-list tails).

### Level 2 — Persistent Collections Working Together

Tests vectors, maps, and sets interoperating with higher-order functions. Requires `(fiat Lux)`, Module System v0, persistent collection types, and collection literal syntax. Four programs: frequency map from a list (validates `Map/get`, `Map/put`, `fold`), group-by (validates higher-order + map construction with nested data), set-driven filtering (validates `has?` as a predicate with `filter`), and indexing vector records by key (validates `Vector/to-list` at the list/vector boundary, then cross-collection operations through `group-by`).

### Level 3 — String Processing Pipelines

Tests the String module, threading macros, and Unicode decomposition. Requires threading desugar (`->`, `->>`). Four programs: slugify (validates thread-first chaining with `String/downcase`, `String/replace`), palindrome check (validates `as-codepoints` and `reverse`), word extraction by length (validates thread-last with `filter` and closure), and Caesar cipher (validates `as-codepoints`, `%`, `>=`, `<=`, codepoint arithmetic, and `from-codepoints`).

### Level 4 — Result Types and Error Pipelines

Tests result type handling with maps as result containers and higher-order error threading. Establishes the `{:ok value}` / `{:err reason}` convention. Three programs: safe division, result chaining with short-circuit on error, and a multi-step config parser that validates required fields.

### Level 5 — Mutual Tail Recursion and State Machines

Tests TCO across mutually recursive functions. Depends on TCO machinery, Lux, maps, sets, `Map/get`, `Map/put`, and `has?`. An NPC dialogue state machine where each state is a function that tail-calls the next state. Without proper TCO this overflows; with it, arbitrarily deep state transitions run in constant stack space.

### Level 6 — Self-Hosting Combinator Derivation

Tests that the eight primitives plus arithmetic are sufficient to derive useful abstractions. This level must be self-contained: any helper used by the programs (`length`, `reverse`, `not`, predicates like `odd?`) must be defined earlier in the same benchmark file. No Lux import is allowed. Derives `take`, `drop`, `nth`, `zip-with`, `any?`, `all?`, `partition`, and a full mergesort.

### Level 7 — Orpheus Game Entity System

Full integration test exercising maps as records, immutable state transformation, set membership for tags, threading macros, and closures together. Requires `Math/sqrt`, `Int/to-string`, `and`, `not`, `max`, `min`. Builds entity constructors, pure state transforms (damage, heal, move), distance-based collision detection, per-tick combat processing, and simulates a game tick with a player, enemies, and items.

### Level 8 — Scripting File Processing Pipeline

Tests Firmamentum bindings and real I/O. Requires `Fs/read`, `Fs/write`, `Int/parse`, `Int/to-string`, `max`, `min`. Reads a CSV file, parses records, computes per-department salary statistics using `group-by` and `fold`, formats a report, and writes it to disk. Validates the full read → transform → write scripting pipeline.


## Development Priorities

Each numbered milestone should be implementable without depending on milestones below it. Checkboxes correspond to branches that, when merged, check off the box. Benchmark-level gates are marked in bold.

### 0. Kernel Value Types and Environment

These are the foundational Rust types that every other component depends on.

- [x] `Value` enum with `Nil`, `Bool`, `Int`, `Float`, `String`, `Symbol`, `Keyword`, `List`, and `Function` variants
- [x] `Cons` struct with `head: Value` and `tail: Value`
- [x] `list_from_vec` helper: fold a `Vec<Value>` right-to-left into a cons chain
- [x] `Display` for `Value` with cons-chain-aware list printing
- [x] Rust-side `PartialEq` for test assertions and internal comparison; Fiat-level `is?` remains atom-only
- [x] `Env` struct with `RefCell<HashMap<InternedSymbol, Value>>` bindings and `Rc<Env>` parent chain
- [x] Named `fiat` inserts the function into the captured environment for recursion
- [x] Symbol lookup through the parent chain
- [x] Runtime `Error` type with constructors for common errors

### 1. Reader v0 and Printer v0 [Levels 0–1]

Reader v0 is enough to pass Levels 0 and 1. Collection literal syntax is deferred to Reader v1 when the corresponding runtime types exist.

- [x] Tokenizer: integers, floats, strings, symbols, booleans, nil
- [x] Keyword tokenizer: colon-prefixed atoms (`:ok`, `:err`, `:player`, `:dept`)
- [x] List reader: `(` delimited, recursive descent, produces cons chains via `list_from_vec`
- [x] Quote shorthand: `'x` → `(behold x)` handled in the reader
- [x] Comment stripping: `;;` to end of line
- [x] Cons-chain-aware printer: walk `head`/`tail` links, emit `(a b c)` form
- [x] Round-trip fidelity: `print(read(source)) == source` for atoms, lists, keywords

### 2. Program Evaluator and Level 0 Core

This milestone brings the evaluator online with the eight primitives, arithmetic, and program-level evaluation of multiple forms.

- [x] Program = sequence of forms evaluated in order in the same top-level environment
- [x] Self-evaluating atoms: numbers, strings, keywords, booleans, nil
- [x] Symbol evaluation via environment lookup
- [x] Function application: evaluate operator, evaluate arguments, apply
- [x] `behold` — return argument unevaluated
- [x] `choose` — multi-armed conditional with ordered test-expression pairs
- [x] `atom?` — atom predicate (`Nil` is atomic, `List` is not)
- [x] `is?` — atom pointer identity test; error on collection arguments
- [x] `first` — read cons cell head; error on `Nil`
- [x] `rest` — read cons cell tail; `Nil` on `Nil`
- [x] `bind` — allocate one cons cell, O(1); reject non-list tails
- [x] `fiat` — anonymous function declaration with closure capture
- [x] `fiat` — named function declaration with self-reference via `RefCell` environment
- [x] `fiat` body supports one or more forms; final form is returned
- [x] Local named `fiat` declarations are visible to later forms in the same body
- [x] `fiat` shape dispatch: `(fiat CapitalName)` → module import (stub error for now); `(fiat name (params) body)` → named function; `(fiat () (params) body)` → anonymous function
- [x] Numeric primitives (Rust): `+`, `-`, `*`, `/`, `%`, `>`, `<`, `=`
- [x] Derived numeric functions (prelude): `>=`, `<=`, `max`, `min`
- [x] **Level 0 benchmarks pass**

### 3. Minimal Desugar Pass and Level 1 Closures

The desugar pass is not the full hygienic macro system. It is a small, explicit source-to-source transformation used to unblock early benchmark levels.

- [x] `let` desugaring (sequential): `(let ((x v) (y w)) body...)` → `((fiat () (x) ((fiat () (y) body...) w)) v)` — each binding is visible to subsequent bindings
- [x] `not` available as prelude function or builtin
- [x] `and` desugars to nested `choose` forms: `(and a b)` → `(choose (a b) (true false))`
- [x] `or` desugars to `let` + `choose` to avoid double evaluation: `(or a b)` → `(let ((tmp a)) (choose (tmp tmp) (true b)))`
- [x] Lexical closure capture verified (currying, nested closures)
- [x] Functions as values: passable, returnable, storable in data structures
- [x] **Level 1 benchmarks pass**

### 4. TCO Machinery

Tail call optimization must be in place before lists get large. This milestone validates the evaluator's execution strategy but does not by itself complete Level 5, which also depends on collections and module imports.

- [x] Trampoline loop: `eval` returns `TailCall { expr, env }` for tail-position calls
- [x] TCO for self-recursion in `fiat` bodies
- [x] TCO through `choose` branches in tail position
- [x] Verified: 100,000-element list reversal completes without stack overflow

### 5. Reader v1: Collection Literals [Level 2 prerequisite]

Reader v1 extends the reader with collection literal syntax once the corresponding runtime types are about to be implemented.

- [x] Vector literal reader: `[]`
- [x] Map literal reader: `{}` with alternating key/value forms; reject odd element count
- [x] Set literal reader: `#{}`
- [x] Collection literal printing for vectors, maps, and sets

### 6. Module System v0: Lux Import and Namespaced Lookup [Level 2 prerequisite]

Module System v0 provides the minimal infrastructure needed for `(fiat Lux)` and namespaced function calls. Host-registered capability gating is deferred to v1.

- [x] `(fiat CapitalName)` recognized as module import when called with one capitalized symbol
- [x] Module registry for built-in pure-computation modules
- [x] `(fiat Lux)` loads core modules: `Map`, `Vector`, `Set`, `List`, `Int`, `Float`, `Math`, `String`
- [x] Namespaced symbol lookup: `Map/get`, `String/trim`, `Vector/append`
- [x] Importing an unknown module returns a clear error

### 7. Persistent Collections and Level 2

This milestone adds the runtime collection types and the Lux module functions that operate on them.

- [x] `Vector` variant backed by persistent vector (HAMT or `im` crate)
- [x] `Map` variant backed by persistent map
- [x] `Set` variant backed by persistent set
- [x] Collection literals evaluate their elements (Clojure-style)
- [x] Set primitives: `set?`, `has?`, `union`, `intersect`, `without`
- [x] `Map` module: `Map/get`, `Map/put`, `Map/merge`, `Map/entries`, `Map/map-values`
- [x] `Vector` module: `Vector/append`, `Vector/nth`, `Vector/to-list`
- [x] `Vector/to-list` verified: Level 2d uses it to convert vector records before passing to list-based `group-by`
- [x] **Level 2 benchmarks pass**

### 8. String Module and Threading Desugar [Level 3]

This milestone adds string processing and the threading operators to the desugar pass.

- [x] `String` module: `split`, `trim`, `downcase`, `upcase`, `replace`, `concat`, `length`, `join`, `starts-with?`
- [x] `as-graphemes` — string to cons chain of single-grapheme strings
- [x] `as-codepoints` — string to cons chain of integer codepoints
- [x] `as-bytes` — string to cons chain of byte values
- [x] `from-codepoints` — cons chain of codepoints to string
- [x] `->` thread-first desugaring: insert threaded value as first argument
- [x] `->>` thread-last desugaring: insert threaded value as last argument
- [x] `name->` thread-as desugaring: insert wherever the binding name appears
- [x] Per-step operator override within a threading pipeline
- [x] **Level 3 benchmarks pass**

### 9. Result Type Conventions [Level 4]

No new interpreter machinery needed — Level 4 validates that existing maps, `choose`, and higher-order functions compose correctly for error handling patterns.

- [x] `{:ok value}` / `{:err reason}` convention works with existing `Map` and `choose`
- [x] Higher-order `then` chaining with short-circuit verified
- [x] **Level 4 benchmarks pass**

### 10. Mutual Tail Recursion and State Machines [Level 5]

Level 5 depends on TCO machinery (milestone 4), Lux imports, maps, sets, `Map/get`, `Map/put`, and `has?` — all of which exist by this point.

- [x] TCO for mutual recursion across separate functions
- [x] NPC dialogue state machine benchmark passes
- [x] **Level 5 benchmarks pass**

### 11. Self-Hosting Stress Test [Level 6]

Level 6 must be self-contained — any helper function used (`length`, `reverse`, `not`, `odd?`) must be defined in the benchmark file itself. No Lux import is allowed.

- [x] Level 6 benchmark file is self-contained with no prelude dependencies
- [x] `take`, `drop`, `nth`, `zip-with`, `any?`, `all?`, `partition` derived from primitives
- [x] Mergesort derived from primitives
- [x] **Level 6 benchmarks pass**

### 12. Game Entity Integration [Level 7]

Full integration of all features. Requires `Math/sqrt`, `Int/to-string`, `and`, `not`, `max`, `min` to be available.

- [x] Entity constructors using maps, set tags, nested map state
- [x] Pure state transforms: damage, heal, move, inventory
- [x] Collision detection with `Math/sqrt`, distance calculations
- [x] Per-tick update loop with `fold` over entity lists
- [x] **Level 7 benchmarks pass**

### 13. Full Hygienic Macro Expander

The full macro expander replaces the minimal desugar pass where appropriate. This is the transition from bootstrap pipeline to final pipeline.

- [ ] Syntax objects: s-expressions annotated with scope information
- [ ] Hygienic renaming to prevent variable capture across macro boundaries
- [ ] User-defined macros via the macro system
- [ ] Macro expansion as a distinct phase replacing the desugar pass

### 14. Module System v1: Host-Registered Capabilities and Firmamentum [Level 8]

Module System v1 adds the host-registration mechanism that enables capability gating for embedded contexts.

- [x] Host can register or omit modules at initialization
- [x] `(fiat Firmamentum)` only succeeds when the host registers it
- [x] `Fs/read` and `Fs/write` — file I/O bindings
- [x] `Process`, `Net`, `Http` module stubs
- [x] Capability gating verified: embedded context cannot access unregistered modules
- [x] **Level 8 benchmarks pass**

### 15. Memory Management Refinement

Memory management cleanup and cycle-handling after the interpreter is functionally complete.

- [ ] `Rc`-based reference counting for all heap-allocated values
- [ ] Creation-time cycle tracking for recursive `fiat` bindings
- [ ] Cycle group tagging and cooperative deallocation
- [ ] No `unsafe` in the interpreter core


## Running the Benchmarks

Each benchmark level is both a Rust integration test and a standalone `.fiat` file. The file runner evaluates a sequence of forms in a shared top-level environment and can assert the value of the final form. Each benchmark file should either contain one final assertion-producing expression or the Rust integration test should evaluate individual forms and cases separately. The following commands run specific levels or the entire suite.

```bash
cargo test level0    # primitives only
cargo test level1    # closures and higher-order functions
cargo test level7    # full entity system integration
cargo test           # all levels
```

To run a benchmark as a Fiat script (once the file runner exists):

```bash
cargo run -- benchmarks/level0.fiat
cargo run -- benchmarks/level7.fiat
```


## Final Architecture Notes

The following sections describe aspects of the final interpreter architecture that are not needed during the bootstrap phase but are important design goals. They are placed here so they do not distract from the immediate build path above.

### Final Macro Expander Architecture

The final macro expander transforms macro invocations into expanded code before evaluation. It implements hygienic renaming with syntax objects (s-expressions annotated with scope information) to prevent variable capture between macro internals and call sites. All macro expansion is compile-time; there is no runtime `eval`. The expander's output is ordinary Fiat code with no macro invocations remaining. Once the full expander is implemented, it replaces the minimal desugar pass — `let`, threading macros, and any other syntactic sugar become proper macro definitions rather than hardcoded transformations.

### Host Boundary and Orpheus Integration

The Rust/Fiat API boundary is a data boundary, not an object boundary. Every value that crosses between Rust and Fiat is an immutable Fiat value, reference-counted and directly owned. There is no handle or ID indirection layer. Orpheus's architecture treats game state as immutable data that flows through Fiat functions — Fiat receives state as values, transforms it, and returns new values. The tree-walking interpreter is a language development tool; the production Orpheus runtime requires a bytecode VM with intrinsics for frame-rate evaluation.

### Memory Cycle Tracking

Because data is immutable by default, cycles can only form in a small number of places: recursive `fiat` bindings (where a closure captures the environment containing its own name) and explicit mutation operations. At each cycle-capable operation, the interpreter checks whether the value being stored contains a reference back to the container. When a cycle is detected, the involved objects are tagged as a cycle group. On deallocation, if a group's external reference count reaches zero, the entire group is freed. Objects not involved in any cycle pay no cost beyond normal `Rc` behavior.


## Architectural Decisions and Their Rationale

This section records the key design decisions and the reasoning behind each, for reference when revisiting trade-offs later.

**Cons cells for lists, persistent vectors for indexed sequences.** Lists and vectors serve different roles in Fiat. Lists (`()` syntax) are for recursive decomposition via `bind`/`first`/`rest` and for code-as-data (homoiconicity). Cons cells make all three list primitives O(1). Vectors (`[]` syntax) are for indexed ordered collections using persistent HAMT-backed structures with O(log32 n) access and structural sharing. An array-backed list representation would make `bind` and `rest` O(n), turning apparently-linear recursive algorithms into quadratic ones.

**Bootstrap pipeline before full macro expander.** The full hygienic macro system is substantial to implement and depends on a working evaluator. Rather than blocking all progress on macro expansion, the bootstrap pipeline uses a minimal desugar pass that handles `let` and threading macros as hardcoded source-to-source transformations. This gets the evaluator through Levels 0–7 before the macro expander is needed.

**`RefCell` for environment bindings.** Named recursive functions require the function's closure to capture an environment that contains a binding to the function itself. With an immutable `HashMap`, this circular reference cannot be established. `RefCell` provides interior mutability so the function can be inserted into the environment after the closure captures it.

**Tree-walking interpreter first, bytecode VM later.** The tree-walker minimizes distance between language semantics and implementation. Each AST node maps directly to an evaluation rule. Changes to semantics are localized edits to the evaluator, not cascading changes across a compiler and VM.

**`Rc` instead of tracing GC.** Aligns with Rust's ownership model. No `unsafe`, no manual root tracking, no GC pauses, no interaction with the borrow checker. Creation-time cycle tracking handles the one weakness of reference counting by exploiting immutability constraints.

**Persistent data structures instead of copying.** Immutability-by-default requires structural sharing to remain efficient. "Updating" a persistent collection allocates only the nodes along the path to the changed element; everything else is shared.

**No runtime `eval`.** Macro expansion is compile-time only. This keeps the runtime simple, makes sandboxing straightforward, and lets tooling operate on expanded code with full knowledge of what the program will do.

**Collection literals evaluate their elements.** Following Clojure's model, unquoted collection literals evaluate their contents. This is required because benchmark programs use expressions and variable references inside map and vector literals. Quoted literals (inside `behold`) remain inert data.

**Sequential `let` bindings.** `let` desugars to nested anonymous functions so that each binding is visible to subsequent bindings in the same `let` block. This matches Scheme's `let*` semantics rather than Scheme's parallel `let`. The sequential model is required because benchmark programs routinely reference earlier bindings in later value expressions (e.g., computing `new-hp` from `max-hp` in the same `let`).

**List-only sequence functions.** `fold`, `map`, and `filter` are self-hosted Fiat functions built on the list primitives `first`, `rest`, `atom?`, and `bind`. They operate on cons-cell lists, not on vectors. This follows directly from two architectural commitments: the self-hosting property (these functions are defined in Fiat using the eight primitives, which are cons-cell operations) and the deliberate separation of lists and vectors (lists for recursive decomposition, vectors for indexed access). Making these functions polymorphic would require either promoting them to Rust builtins — inflating the kernel and undermining the self-hosting story that Level 6 validates — or extending `first`/`rest` to accept vectors, which would blur the architectural distinction between the two sequence types. Code that needs to iterate over a vector converts it to a list first using `Vector/to-list`, making the representation change explicit.

**`is?` uses pointer identity.** `is?` compares atoms by reference, not by value. This is reliable for inline types (numbers, booleans, nil) where identity and value equality coincide, and for interned types (symbols, keywords) where equal names share a single allocation. For strings, `is?` compares `Rc` pointers — separately constructed strings with identical content are not `is?`-equal. Value comparison for strings uses `String` module functions. This keeps the primitive semantics simple and predictable.

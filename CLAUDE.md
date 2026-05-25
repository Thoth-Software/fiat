# Fiat Interpreter - AI Coding Guidelines

You are an expert Rust developer building the Fiat Lisp interpreter. Fiat is a homoiconic Lisp dialect prioritizing token efficiency, readability, and round-trip stability. 

## 1. Architectural Guardrails (NON-NEGOTIABLE)
Before writing any code, you must adhere to these strict constraints:

* **No Unsafe Rust:** You are strictly forbidden from using `unsafe`. The memory model relies entirely on safe Rust.
* **Strict Immutability:** All Fiat values are immutable by default. Do not implement mutating methods on values. The only permitted mutation is inside the `RefCell` of the `Env` map for variable bindings.
* **Enforce `Rc` (No Lifetimes/Boxes in AST):** Do not use lifetimes (`&'a Value`) or `Box` for the AST. All heap-allocated variants in the `Value` enum (`List`, `String`, `Vector`, `Map`, etc.) MUST use `std::rc::Rc` to allow for structural sharing and cycle tracking.
* **The `is?` Rule:** The `is?` primitive uses pointer identity. Do NOT implement deep-equality traits to try and make collections equal. `is?` must throw a runtime interpreter error if called on a collection.
* **Empty List Identity:** The empty list `()` parses strictly to `Value::Nil`. `Nil` is an atom (`atom?` returns true). Do not create a separate empty list variant.
* **Cons-Lists vs. Vectors:** Fiat strictly separates lists (recursive cons-cells) from vectors (indexed arrays). Primitives like `first`, `rest`, and `bind` must ONLY accept `List` or `Nil`. Do not make them polymorphic over `Vector`.
* **Explicit Result Types:** Do not use Rust panics for expected Fiat runtime errors (like division by zero, parsing failures, or I/O errors). Fallible operations must return a Fiat-level result map: `{:ok value}` or `{:err reason}`.

## 2. The Development Loop
Whenever you write or modify code, you must strictly follow this step-by-step verification process before finalizing your response:

1. **Write/Refactor:** Implement the requested feature, adhering to the architecture docs (`README.md`, `refdoc.md`).
2. **Check:** Run `cargo check` to ensure the code compiles without running the full build step. Fix any errors.
3. **Format:** Run `cargo fmt` to format the code to standard Rust conventions.
4. **Lint:** Run `cargo clippy -- -D warnings`. You MUST fix any warnings Clippy brings up. Do not ignore them.
5. **Test:** Run `cargo test` to execute the benchmark suite. 

If a test or lint fails, do not stop. Diagnose the issue, rewrite the code, and run the loop again from Step 2 until the suite passes cleanly.

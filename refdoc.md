# Fiat — Language Reference

> **Note:** This document specifies the *completed* Fiat language. The features described here represent the final design target, written in the present tense for clarity. The current interpreter may not yet implement every component (for example, the bootstrap pipeline desugars `let` and the threading forms rather than expanding them through a hygienic macro expander). For the state of the implementation and the milestone roadmap, consult the README.

## What Fiat Is

Fiat is a homoiconic Lisp dialect with its core implemented in Rust. It targets two use cases: general-purpose scripting (replacing bash, Python, Perl) and embeddable scripting for the Orpheus game engine (occupying the same role as Lua).

Fiat's design priorities, in order, are: token efficiency for LLM generation, readability for human inspection, and round-trip stability for the cycle of generation → reading → editing → re-generation. It is not optimized for the act of typing.

The total primitive count is eight special forms plus arithmetic and set operations — 21 operations from which the entire language is built. Fiat self-hosts the prelude and all abstractions that can be expressed in terms of the primitive kernel. Representation-sensitive operations on built-in data types, and host capabilities, may be provided as Rust module bindings. The Level 6 benchmark validates the self-hosted subset.


## Architecture: Three Layers

Fiat's implementation separates into three layers with distinct responsibilities:

**Embedding core.** The evaluator, type system, macro expander, lexical scoping, closures, and host API boundary. This is the minimal interpreter that both use cases share. It has no I/O, no filesystem access, no networking — it can only compute.

**Host-side scripting bindings.** Rust functions that provide OS-level capabilities (file I/O, process spawning, networking, regex, etc.), registered into the Fiat environment by the host application. These are plain functions, not macros — they do the actual work via system calls and Rust libraries.

**Fiat-side scripting prelude.** A set of macros, written in Fiat, that provide ergonomic surface syntax over the host bindings. A `pipe` macro, shell-like conveniences, pattern matching sugar, threading macros, and similar forms live here.

This separation gives clean capability gating. The embedding core is safe to expose to untrusted code (game mods, user scripts) because it literally cannot do anything outside of computation. The host decides which bindings to register — Orpheus grants access to game-specific APIs but not the filesystem; the standalone scripting runtime grants everything. The macro prelude's forms don't even exist unless the corresponding host bindings are present, so there is no surface to accidentally expose.


## Values and Types

Every value in Fiat is represented as a variant of a single Rust enum:

```rust
enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<str>),
    Symbol(InternedSymbol),
    Keyword(InternedSymbol),
    List(Rc<Cons>),
    Vector(Rc<PersistentVector<Value>>),
    Map(Rc<PersistentMap<Value, Value>>),
    Set(Rc<PersistentSet<Value>>),
    Function(Rc<Function>),
}

struct Cons {
    head: Value,
    tail: Value,    // must be Nil or List(Rc<Cons>)
}
```

Exact enum size is an implementation detail and should be measured with `std::mem::size_of::<Value>()` once the enum stabilizes. For heap-allocated types (strings, lists, vectors, maps, sets, functions), the payload is a reference-counted pointer; the actual data lives on the heap.

Values fall into two broad categories:

**Atoms** are indivisible values. Nil, booleans (`true`, `false`), integers (`42`, `-7`), floats (`3.14`), strings (`"hello"`), symbols (`player`, `+`), and keywords (`:player/hp`) are all atoms. The empty list `()` is equivalent to `Nil` and is atomic. Atoms are the leaves of every data structure.

**Collections** are compound values. Lists, vectors, maps, and sets are all collections. They nest to arbitrary depth and contain any mix of value types.


## Surface Syntax

Fiat's surface syntax takes Clojure as its point of departure. Distinct bracket types distinguish data structures at a glance:

- `()` — lists and function calls
- `[]` — vectors (persistent, indexed)
- `{}` — maps (persistent, key-value)
- `#{}` — sets (persistent, unordered)

Items within collections are separated by whitespace. Commas are not used as separators.

These literal forms are recognized by the reader, which uses the bracket types to construct the corresponding `Value` variants — `Value::Vector`, `Value::Map`, `Value::Set` — without requiring macro expansion or desugaring into function calls. However, the reader does not evaluate the elements inside a collection literal. It constructs syntax values containing raw, unevaluated forms. Element evaluation is the evaluator's responsibility and happens when the collection literal is evaluated in a context outside `behold`. See "How Evaluation Works" for the full rules.


## How Evaluation Works

Fiat evaluates expressions using the following rules. An atom that is a number, string, keyword, boolean, or nil evaluates to itself. An atom that is a symbol evaluates to whatever value is bound to that name in the current environment. A list evaluates by treating its first element as an operator: if it names a special form (`fiat`, `behold`, `choose`), that form's evaluation rules apply; otherwise the first element is evaluated as a function and applied to the remaining elements, which are each evaluated first.

For example, `(+ 1 2)` evaluates as follows: `+` is looked up and found to be the addition function, `1` evaluates to `1`, `2` evaluates to `2`, and the function is applied to produce `3`. The expression `(double (+ 1 2))` evaluates the inner expression first, producing `3`, then applies `double` to `3`.

Collection literals — vectors (`[]`), maps (`{}`), and sets (`#{}`) — evaluate their elements when they appear in evaluated code. This follows Clojure's model: symbols and expressions inside an unquoted collection literal are evaluated, producing a collection of computed values. Quoted collection literals (inside `behold`) are inert data — their elements remain unevaluated.

```lisp
(let ((x 5))
  [x (+ x 1) (+ x 2)])    ;; → [5 6 7], elements evaluated

(behold [x (+ x 1)])       ;; → [x (+ x 1)], elements unevaluated
```

Map literals use alternating key/value forms. A map literal with an odd number of forms is a reader error:

```lisp
{:name "Alice" :dept :eng}    ;; two key-value pairs
{:x (+ 1 2) :y (* 3 4)}      ;; keys are keywords, values are evaluated → {:x 3 :y 12}
```


## Scoping

Variable references in Fiat are resolved lexically — a variable refers to the binding in the closest enclosing scope where it was defined, determined at compile time by the textual structure of the program. This is in contrast to dynamic scoping, where variable lookup walks the call stack at runtime.

Lexical scoping is what makes closures predictable. When a function captures a variable from its enclosing scope, that binding is fixed at the time the function is created, regardless of where the function is later called. With dynamic scoping, the same function could see different values for the same variable name depending on who called it — a behavior that makes programs extremely difficult to reason about and is widely regarded as a historical mistake in early Lisps.

Lexical scoping also simplifies cycle tracking (see the Memory Management section). Closures capture a reference to a specific, statically-known environment frame. The interpreter can walk this chain at binding time to detect cycles precisely because the chain is determined by program structure, not by runtime call patterns.


## Conventions

Fiat enforces the following naming and stylistic conventions to maintain consistency across codebases and improve readability for both humans and LLMs.

**Boolean-returning functions end in `?`.** Any function that returns a boolean value must have a name ending in a question mark. This applies to primitives (`atom?`, `is?`, `has?`, `set?`) and to user-defined functions alike. A function named `colliding?` returns a boolean. A function named `colliding` does not. The `?` suffix is not decorative — it is a reliable signal of return type, visible at every call site without inspecting the function body.

**All symbols ending in `->` are reserved for threading syntax.** The pattern `*->` (any identifier suffixed with `->`) is recognized by the macro expander as a threading operator (see the Threading Macros section). This reservation is unambiguous and is documented as part of the language specification to prevent accidental conflicts.


## The Eight Primitives

Fiat has eight primitive operators, all implemented in Rust. Everything else in the language is derived from them. They group into natural pairs and roles:

| Primitive | Role | Pair |
|-----------|------|------|
| `behold` | Arrest an expression — return it as data, unevaluated | (metaprogramming) |
| `first` | Return the head of a list | ↔ `rest` |
| `rest` | Return everything after the head | ↔ `first` |
| `bind` | Join a value to a list, constructing a new list | (inverse of first/rest) |
| `atom?` | Test whether a value is an atom | ↔ `is?` |
| `is?` | Test whether two atoms are identical | ↔ `atom?` |
| `choose` | Multi-armed conditional | |
| `fiat` | Declare functions and import modules | |


## behold — Suspending Evaluation

`behold` takes a single argument and returns it without evaluating it. The argument becomes inert data — a list or atom that can be inspected, decomposed, and transformed, but will not be executed.

```lisp
(behold (+ 1 2))       ;; → the list (+ 1 2), not 3
(behold foo)            ;; → the symbol foo, not its bound value
(behold (a b c))        ;; → the list (a b c)
```

`behold` is the foundation of homoiconicity. Without it, expressions can only be executed. With it, expressions can be held as values — passed as arguments, stored in data structures, and transformed structurally. The shorthand `'x` can be used for `(behold x)`.

`behold` serves compile-time metaprogramming: macro definitions use it to construct and manipulate code as data during macro expansion. There is no corresponding runtime eval — code transformation and execution happen at compile time through the macro system, not at runtime through a general-purpose `eval` primitive. This keeps the runtime simple and predictable: the set of operations a program can perform is fixed after macro expansion, sandboxing is straightforward, and tooling like static analysis and debuggers can operate on expanded code with full knowledge of what the program will do.


## first and rest — Decomposing Lists

`first` returns the first element of a list. `rest` returns everything after the first element. Together they allow recursive traversal of any list structure.

```lisp
(first '(a b c))           ;; → a
(first '((a b) c d))       ;; → (a b)

(rest '(a b c))             ;; → (b c)
(rest '(a))                 ;; → ()
```

Every list-processing algorithm in Fiat follows the same pattern: take the head with `first`, process it, recur on the tail with `rest`, terminate when the tail is `()`. These are the destructuring operations — they dissolve a list into its components.


## bind — Constructing Lists

`bind` joins a value to the front of a list, producing a new list. It is the only list constructor in the language. All list structures are built from it.

```lisp
(bind 'a '(b c))           ;; → (a b c)
(bind 'a ())                ;; → (a)
(bind '(x y) '(z))         ;; → ((x y) z)
```

A list like `(a b c)` is internally `(bind 'a (bind 'b (bind 'c '())))` — a chain of `bind` operations terminating in the empty list. The identity law holds: for any non-empty list `x`, `(bind (first x) (rest x))` produces `x`. The three operations `bind`, `first`, and `rest` form a closed algebra over lists.


## atom? and is? — Interrogating Values

`atom?` tests whether a value is an atom (indivisible) rather than a collection. `is?` tests whether two atoms are identical. Both return boolean values and follow the `?` naming convention.

```lisp
(atom? 'foo)                ;; → true (symbol is atomic)
(atom? 42)                  ;; → true (number is atomic)
(atom? '(a b))              ;; → false (list is not atomic)
(atom? ())                  ;; → true (empty list / nil is atomic)

(is? 'foo 'foo)             ;; → true
(is? 'foo 'bar)             ;; → false
```

`is?` is defined only on atoms. Comparing two collections for equality requires structural traversal — collection equality is derived, not primitive. This keeps the kernel minimal.


## choose — Conditional Branching

`choose` is Fiat's only control flow primitive. It takes a sequence of test-expression pairs and returns the expression associated with the first test that evaluates to true.

```lisp
(choose
  ((is? x 'a)  'found-a)
  ((is? x 'b)  'found-b)
  (true         'not-found))
```

Each clause is a list of two elements: a test and a result. Tests are evaluated in order. When a test returns true, the corresponding result is evaluated and returned. No subsequent tests are evaluated. The atom `true` is conventionally used as the final catch-all test.

A two-armed `choose` is equivalent to `if-then-else`. All other branching constructs (case dispatch, pattern matching, guard clauses) are built from `choose` through macros or derived functions.


## fiat — Declaring Functions and Importing Modules

`fiat` is a dual-purpose form. Its role is determined by its argument structure.

### Function Declaration

When given a name (or `()` for anonymous), a parameter list, and one or more body forms, `fiat` creates a function. It replaces both `lambda` (anonymous functions) and `define`/`label` (named, potentially recursive functions) from traditional Lisps.

When multiple body forms are present, they are evaluated in order in the function's local environment, and the value of the final form is returned. This allows local helper definitions without requiring a separate `do` or `begin` form.

**Anonymous functions** use `()` in the name slot. The empty list as a name means the function exists as a value only — it can be passed, returned, and applied, but has no name and cannot call itself recursively.

```lisp
(fiat () (x) (* x 2))               ;; anonymous, one argument
(fiat () (x y) (+ x y))             ;; anonymous, two arguments
((fiat () (x) (* x 2)) 5)           ;; immediately applied → 10
```

**Named functions** place a symbol in the name slot. The name is bound in the enclosing environment and is available inside the body for recursion. A named `fiat` declaration returns the function value, making declarations usable in expression position.

```lisp
(fiat double (x) (* x 2))

(fiat factorial (n)
  (choose
    ((is? n 0) 1)
    (true (* n (factorial (- n 1))))))
```

**Multi-form bodies** allow local definitions to precede the main expression. Local named `fiat` declarations are visible to later forms in the same body:

```lisp
(fiat reverse (lst)
  (fiat go (remaining acc)
    (choose
      ((atom? remaining) acc)
      (true (go (rest remaining) (bind (first remaining) acc)))))
  (go lst ()))
```

### Module Import

When given a single capitalized symbol, `fiat` imports a module:

```lisp
(fiat Lux)                  ;; import the core standard library
(fiat Firmamentum)          ;; import the scripting capability layer
(fiat Orpheus.Input)        ;; import a game-specific module
```

The disambiguation is syntactic and unambiguous: `(fiat CapitalName)` is a module import — one capitalized symbol after `fiat`, nothing else. `(fiat name (params) body...)` is a function declaration — a name (or `()` for anonymous), followed by a parameter list, followed by one or more body forms. These two shapes cannot be confused.

See the Module System section for full details on modules and capability gating.


## Arithmetic

Arithmetic provides `+`, `-`, `*`, `/`, `%`, `>`, `<`, `=` as native operations on numeric atoms. These are implemented in Rust because numeric computation is a primary workload and Church-encoding arithmetic would be impractical.

`%` is the integer remainder operator. Like `/`, it is a hardware-level operation that cannot be derived from the other arithmetic primitives. `(% 10 3)` returns `1`. `(% 7 2)` returns `1`. Division by zero in `%` is a runtime error, same as `/`.

The core numeric types are `i64` (64-bit signed integer) and `f64` (64-bit IEEE 754 float). Both fit inline in the 8-byte `Value` payload — no heap allocation, no pointer indirection. For the tight loops and per-entity calculations common in game scripting, this keeps arithmetic effectively free.

There is no automatic promotion to arbitrary precision. If an `i64` overflows, the result is an error via the result type system — not a silent promotion to a bignum. Arbitrary-precision integers and rational numbers are available as extended standard library modules, provided through host bindings. They are heap-allocated behind `Rc` pointers and available when precision matters more than speed (financial calculations, cryptographic operations, combinatorics), but they are not the default, and code that doesn't import them pays no cost.

### Derived Comparison and Numeric Functions

The following comparison and numeric functions are not primitives. They are defined in the prelude using `choose`, `not`, and the primitive comparison operators, and are available in every Fiat environment once the prelude is loaded:

```lisp
(fiat >= (a b) (not (< a b)))
(fiat <= (a b) (not (> a b)))

(fiat max (a b) (choose ((> a b) a) (true b)))
(fiat min (a b) (choose ((< a b) a) (true b)))
```

`>=` and `<=` are derived from `<` and `>` via `not` — no new Rust implementation is required. `max` and `min` are two-argument functions that return the larger or smaller of their operands using `choose`. All four are used throughout the benchmark suite (Level 3's Caesar cipher uses `>=` and `<=`; Level 7's entity system uses `max` and `min` for health clamping) and are part of the standard environment, but they are not part of the Rust kernel.


## Set Operations

Set operations are implemented in Rust as primitives. They operate on the native persistent set type (`#{}`), which provides O(1) membership testing and efficient union/intersection/difference:

- `set?` — Predicate testing whether a value is a set. `(set? #{a b})` returns `true`. `(set? '(a b))` returns `false`.
- `has?` — Membership test. `(has? 'a #{a b c})` returns `true`. O(1).
- `union` — Set union. `(union #{a b} #{b c})` returns `#{a b c}`.
- `intersect` — Set intersection. `(intersect #{a b c} #{b c d})` returns `#{b c}`.
- `without` — Set difference. `(without #{a b c} #{b})` returns `#{a c}`.


## Persistent Data Structures and Structural Sharing

Fiat's built-in collection types — vectors, maps, and sets — are persistent data structures. "Updating" a persistent collection returns a new collection that shares the vast majority of its memory with the original. Only the nodes along the path to the changed element are newly allocated; everything else is shared via reference counting.

This is essential for the immutable-by-default architecture to remain efficient. When Orpheus passes game state through Fiat functions each tick, most of that state doesn't change — with structural sharing, the "new" state returned by Fiat is almost entirely composed of pointers into the previous state. The cost of producing new state is proportional to what actually changed, not to the total size of the state.

The canonical implementation is the hash array mapped trie (HAMT), as used by Clojure and Scala's immutable collections. A HAMT provides O(log32 n) lookup, insertion, and removal — effectively constant time for practical collection sizes — while sharing structure with previous versions.

Without persistent data structures, immutability-by-default would require full copies on every "update," making the architecture impractical for any non-trivial state size. This is a load-bearing decision — the viability of immutability-by-default and the owned-reference host boundary depends on it.

Lists (`()` syntax) remain traditional cons-cell chains for simplicity and compatibility with the `bind`/`first`/`rest` primitives. For indexed access or "update" operations on ordered sequences, vectors (`[]` syntax) are the appropriate choice.


## Immutability by Default

All data in Fiat is immutable by default. Lists, vectors, maps, sets, strings, and other compound values cannot be modified after creation. Mutation is available through explicit, opt-in constructs but is not the norm.

This eliminates a large class of bugs (aliasing problems, iterator invalidation, race conditions in any future concurrency model), makes reasoning about program behavior easier, and constrains the number of operations that can create reference cycles to a small, well-defined set (see Memory Management).


## Strings

Strings are internally represented as UTF-8 byte sequences (`Rc<str>` in the `Value` enum) but are opaque to Fiat code — they do not support positional indexing. There is no `(nth my-string 5)` operation. Instead, strings are operated on as whole values through functions: `String/split`, `String/trim`, `String/upcase`, `String/replace`, `String/starts-with?`, `String/concat`, and similar.

When positional or per-character access is needed, the user explicitly converts to a list using a function that makes the abstraction level visible:

- `as-codepoints` — returns a list of integer Unicode scalar values. Each element is a single codepoint. Appropriate for text processing where encoding-level precision matters.
- `as-graphemes` — returns a list of single-grapheme strings. Each element is one visual character as perceived by a human reader, determined by the Unicode grapheme segmentation algorithm. Appropriate for user-facing text manipulation.
- `as-bytes` — returns a list of integer byte values. The raw UTF-8 encoding. Appropriate for binary protocol work or encoding-level manipulation.

The inverse conversion rebuilds a string from codepoints:

- `from-codepoints` — the inverse of `as-codepoints`: takes a list of integer codepoints and returns the corresponding string. Each integer must be a valid Unicode scalar value; an out-of-range or negative integer, or a non-integer element, raises an error. For example, `(from-codepoints '(72 101 108 108 111))` yields `"Hello"`, and `(from-codepoints (as-codepoints s))` round-trips any string `s`.

This design avoids a problem that has no good default answer: what "the fifth character" means in a Unicode string. Rather than choosing a default that will be wrong in some contexts, Fiat makes the choice explicit. The cost of each abstraction level is visible in the code, and the threading macro `->` makes the conversion step read naturally as part of a pipeline:

```lisp
;; count visible characters
(-> name as-graphemes length)

;; get the first letter, uppercased
(-> name as-graphemes first String/upcase)
```


## Error Handling: Result Types

Fiat uses result types rather than exceptions or Lisp-style conditions for error handling. Operations that can fail return a value that explicitly represents either success or failure. The caller must handle both cases — failure is part of the function's return type, not an invisible control flow path.

Exceptions are a form of hidden control flow: any function call might throw, and the only way to know which ones is documentation or experience. Error paths should be visible in the code. Result types make error handling explicit, composable, and impossible to accidentally ignore.

This aligns with the immutability-by-default philosophy. Exceptions rely on stack unwinding, which is inherently imperative — it mutates the program counter and destroys stack frames as a side effect. Result types are values like any other: they can be passed, stored, pattern-matched, and transformed with ordinary functions.

The practical cost is verbosity — without syntactic support, result handling can become nested and awkward. This is where the macro system earns its keep. A threading macro that short-circuits on error (analogous to Rust's `?` operator or Haskell's monadic `do` notation) makes result-type code as concise as exception-based code while retaining the explicitness.


## Tail Call Optimization

Fiat guarantees tail call optimization (TCO). Any function call in tail position — the last expression evaluated before a function returns — reuses the current stack frame instead of allocating a new one. Tail-recursive functions run in constant stack space regardless of iteration count.

This is effectively mandatory. With immutable data and no mutation-based loop constructs, recursion is the primary mechanism for iteration. Without TCO, a simple loop processing a list of 10,000 elements would require 10,000 stack frames and overflow. TCO transforms tail recursion into the equivalent of a `goto` back to the top of the function — iteration in functional clothing.

Proper TCO also extends to mutual recursion (A calls B in tail position, B calls A in tail position), which is important for state-machine-style code common in game scripting — dialogue systems, AI behavior, animation controllers — where each state is a function that transitions to another state by tail-calling it.


## Macros: Compile-Time Metaprogramming

Fiat's homoiconicity is used for a macro system that transforms code at compile/read time. Programs can define macros that rewrite s-expressions into other s-expressions before evaluation. Fiat does not support runtime code generation — there is no `eval` that executes arbitrary constructed code at runtime.

This preserves the benefits of homoiconicity (user-defined syntax, DSLs, boilerplate elimination) while keeping the runtime simple and predictable. Without runtime `eval`, the interpreter doesn't need to carry the macro expander into runtime, sandboxing is straightforward (the set of operations a program can perform is fixed after expansion), and tooling like static analysis and debuggers can operate on expanded code with full knowledge of what the program will do.

### Hygiene

Fiat's macro system is hygienic. Macro-introduced variable names cannot accidentally capture or shadow variables at the macro's call site, and variables at the call site cannot leak into the macro's internal bindings. Each macro expansion operates in its own lexical scope, and the macro system enforces this automatically.

The alternative — unhygienic macros, as in Common Lisp's `defmacro` — gives the macro author full control over the output s-expression with no scope isolation. This is powerful but error-prone: a macro that internally uses a variable called `result` will silently shadow any `result` variable in the caller's scope. The conventional workaround (`gensym`) is manual discipline the system doesn't enforce.

Hygienic macros eliminate this class of errors by construction. The macro expander tracks which scope each identifier belongs to and renames as necessary to prevent collisions. The implementation requires syntax objects (s-expressions annotated with scope information) and a renaming or marks-based algorithm — substantial complexity, but a one-time cost that permanently eliminates an entire category of macro bugs.

This is especially important because the scripting prelude is built from macros, and users will write their own macros for game-specific DSLs. Hygiene makes macros safe to compose, which is essential when macros are a core extension mechanism.


## Threading Macros

Fiat provides threading macros that combine Clojure's threading model with per-step operator switching. The base operators are:

- `->` — thread-first. Inserts the threaded value as the first argument of each form.
- `->>` — thread-last. Inserts the threaded value as the last argument of each form.
- `name->` — thread-as. Inserts the threaded value wherever `name` appears in the form. The binding name is encoded in the operator itself: `val->` binds as `val`, `it->` binds as `it`, etc.

The outer operator sets the default threading mode for the entire pipeline. Any individual step can override the mode by prefixing itself with a different operator. The override applies to that step only — subsequent steps revert to the default.

```lisp
;; Default is thread-first. Step 3 overrides to thread-last.
(-> x
    (+ 2)            ;; ->:  (+ x 2)
   ->> (- 3)         ;; ->>: (- 3 result)
    (* 5)            ;; ->:  (* result 5)
    (% 7))           ;; ->:  (% result 7)

;; Default is thread-last. Step 2 overrides to thread-first.
(->> some-list
     (filter odd?)   ;; ->>: (filter odd? some-list)
  -> (nth 3)         ;; ->:  (nth result 3)
     (String/concat "got: "))  ;; ->>: (String/concat "got: " result)

;; Thread-as with explicit placement throughout.
(it-> x
      (+ it 2)
      (some-fn 1 it 3)
      (* it 5))

;; Thread-first default, one step uses explicit placement.
(-> x
    (+ 2)
   val-> (some-fn 1 val 3)
    (* 5))
```

The `name->` form works both as the outer operator (setting the default for all steps) and as a per-step override (affecting only the prefixed step).


## Module System

Modules are the unit of code organization and capability gating in Fiat. A module is imported with `(fiat ModuleName)`, where module names begin with a capital letter. Module names abstract over data types, following the Elixir convention: the `String` module contains functions that operate on strings, the `Map` module contains functions that operate on maps. Functions are namespaced by their module and accessed with `/`: `String/upcase`, `Map/get`, `Set/union`.

The standard library is split into two tiers:

**Lux** — the curated core. Contains modules for Fiat's built-in data types: `String`, `Map`, `Set`, `Vector`, `List`, `Int`, `Float`, and other pure-computation modules. These operate on core `Value` types, perform no I/O, require no OS-level capabilities, and are available in every Fiat environment. Every Fiat program begins with `(fiat Lux)`.

**Firmamentum** — the scripting capability layer. Contains modules that interact with the operating system: `Fs` (filesystem), `Process` (spawning and managing processes), `Net` (networking), `Http` (HTTP client), and similar. These are host-provided bindings bundled with their ergonomic macro surfaces. `Firmamentum` is only available when the host registers it — in standalone scripting mode, it is present; in embedded contexts like Orpheus, it does not exist.

A standalone script begins with both imports:

```lisp
(fiat Lux)
(fiat Firmamentum)
```

An Orpheus game mod imports the core and game-specific modules only:

```lisp
(fiat Lux)
(fiat Orpheus.Input)
(fiat Orpheus.State)
```

Attempting to import a module the host has not registered is an error. This is the capability gating mechanism: capabilities are not flags that permit or deny operations at runtime — they are the presence or absence of entire modules. If `Firmamentum` is not registered in the environment, the syntax for filesystem access, process spawning, and networking does not exist. There is no surface to accidentally expose, no permission check to bypass, and no runtime error path for "operation not permitted." The forms are simply not there.

Each module is a self-contained bundle of host-side Rust bindings (the functions that do the actual work) and Fiat-side macros (the ergonomic surface syntax). When the host loads a module, both layers come in together. When the host does not load it, neither layer exists.


## Self-Hosting: Building the Language in Itself

The following derivations show how higher-level constructs can emerge from the primitive set.

**`let` (local bindings)** is syntactic sugar for nested application of anonymous `fiat` forms, where each binding is visible to subsequent bindings. The expression `(let ((x 5) (y (+ x 1))) (+ x y))` desugars to `((fiat () (x) ((fiat () (y) (+ x y)) (+ x 1))) 5)`. No new primitive is needed — this is a macro expansion.

**`not` (boolean negation):**

```lisp
(fiat not (x)
  (choose
    (x false)
    (true true)))
```

**`>=` and `<=` (comparison):**

```lisp
(fiat >= (a b) (not (< a b)))
(fiat <= (a b) (not (> a b)))
```

**`max` and `min` (numeric extremes):**

```lisp
(fiat max (a b) (choose ((> a b) a) (true b)))
(fiat min (a b) (choose ((< a b) a) (true b)))
```

**`map` (apply a function to every element of a list):**

```lisp
(fiat map (f lst)
  (choose
    ((atom? lst) ())
    (true (bind (f (first lst))
                (map f (rest lst))))))
```

**`filter` (keep elements satisfying a predicate):**

```lisp
(fiat filter (pred lst)
  (choose
    ((atom? lst) ())
    ((pred (first lst))
      (bind (first lst) (filter pred (rest lst))))
    (true (filter pred (rest lst)))))
```

**`fold` (reduce a list to a single value):**

```lisp
(fiat fold (f acc lst)
  (choose
    ((atom? lst) acc)
    (true (fold f (f acc (first lst)) (rest lst)))))
```

**`length`** is fold applied with a counter:

```lisp
(fiat length (lst)
  (fold (fiat () (acc x) (+ acc 1)) 0 lst))
```

These derivations demonstrate that the eight primitives plus arithmetic are sufficient for general-purpose programming. The standard library grows from this foundation without requiring additional Rust implementation.


## Memory Management

### Reference Counting

Memory is managed through Rust's `Rc` (reference-counted pointer). When a value is shared (bound to multiple variables, stored in multiple data structures), the reference count tracks how many owners exist. When the last owner goes away, the value is freed immediately.

This aligns with Rust's ownership model rather than fighting it. `Rc` requires no `unsafe` code, no manual root tracking, no GC pauses, and no interaction with the Rust borrow checker. Tracing garbage collection in Rust would require raw pointers, manual GC root tracking at every Rust/Fiat boundary, and `unsafe` throughout the interpreter.

### Creation-Time Cycle Tracking

Rather than running a periodic cycle-detection sweep (as CPython does), Fiat tracks reference cycles at the moment they form.

Because data is immutable by default, cycles can only be created in a small number of places: recursive bindings (`fiat` with a name, `letrec`) where a closure is stored into the same environment it captures, and explicit mutation operations (`set!`), which are rare by convention. Normal value construction — building lists, vectors, maps — cannot create cycles because the components already exist and cannot point back to the container being created.

At each cycle-capable operation, the interpreter checks whether the value being stored contains a reference back to the container receiving it — concretely, walking the closure's captured environment chain to see if it includes the target scope. This is O(scope nesting depth), typically 3–15 pointer comparisons. When a cycle is detected, the involved objects are tagged and registered as a cycle group.

On the deallocation path, when an `Rc` refcount decrements, the interpreter checks whether the object is tagged as part of a cycle group. If so, it checks whether the group's external reference count has reached zero. If it has, the entire group is freed. Objects not involved in any cycle (the vast majority) pay no cost beyond normal `Rc` behavior — one branch to check the tag, which will virtually always be false.


## Host Boundary: Owned References, No Handles

The Rust/Fiat API boundary is a data boundary, not an object boundary. Every value that crosses between Rust and Fiat is an immutable Fiat value, reference-counted and directly owned. There is no handle or ID indirection layer.

In a typical embedding architecture (Lua in a game engine, for example), the scripted language holds opaque handles to host-side objects because the host mutates those objects between script invocations. Fiat doesn't need this because Orpheus's architecture treats game state as immutable data that flows through Fiat functions — Fiat receives state as values, transforms it, and returns new values. There is no long-lived mutable host-side state that Fiat references across ticks.

This eliminates an entire layer of complexity: no handle validation, no generation indices, no lookup tables, no stale-handle error paths.


## The Rust Kernel

The Rust kernel provides the minimal substrate on which Fiat runs. It consists of six components:

**The reader** converts text into s-expressions — the internal representation of atoms, lists, vectors, maps, and sets. The reader defines the grammar: how `()` delimits lists, how `[]` delimits vectors, how `{}` delimits maps, how `#{}` delimits sets, how colons and slashes participate in tag syntax, how numbers are parsed, and how `?` is a valid trailing character in symbols. The reader constructs the appropriate `Value` variants for each syntactic form but does not evaluate the elements inside collection literals — it produces syntax values containing raw, unevaluated forms.

**The printer** converts s-expressions back to text. It is the inverse of the reader and ensures round-trip fidelity.

**The macro expander** transforms macro invocations into expanded code before evaluation. It implements hygienic renaming, tracks syntax objects with scope annotations, and resolves the expansion of all macros — including user-defined macros and the scripting prelude — at compile time. The expander operates on quoted s-expressions (produced by `behold`), and its output is ordinary Fiat code with no macro invocations remaining.

**The evaluator** implements the eight primitives. It recognizes `behold`, `atom?`, `is?`, `first`, `rest`, `bind`, `choose`, and `fiat` as special forms with defined evaluation rules. All other function application follows the default rule: evaluate the function, evaluate the arguments, apply. The evaluator is also responsible for evaluating the elements inside collection literals — when it encounters a vector, map, or set node from the reader, it evaluates each element in the current environment and constructs the resulting collection from the computed values.

**Arithmetic** provides `+`, `-`, `*`, `/`, `%`, `>`, `<`, `=` as native operations on numeric atoms.

**Set operations** provide `set?`, `has?`, `union`, `intersect`, and `without` as native operations on set values.


## Implementation Strategy

The initial implementation uses a tree-walking interpreter: the evaluator recursively walks the abstract syntax tree, evaluating each node directly. This will later be replaced by a bytecode compiler and virtual machine.

Tree-walking minimizes the distance between the language's semantics and the implementation. Each AST node maps directly to an evaluation rule. There is no intermediate representation to design, no instruction set to define, no compiler pass to debug separately from the evaluator. This directness is essential during the phase where the language's semantics are still being refined — changes to how `let` or closures work are localized edits to the evaluator, not cascading changes across a compiler and a VM.

The migration path to bytecode is well-understood: introduce a compilation pass that walks the same AST and emits instructions instead of evaluating directly. The key decisions that make this migration smooth are already in place — lexical scoping means variable references can be resolved to frame offsets at compile time, and tail call optimization maps directly to a `TAIL_CALL` instruction. The language semantics don't change; only the execution strategy does.


## Concurrency: Deferred but Not Precluded

Fiat does not implement concurrency in its initial version, but its architecture must not preclude it.

Immutable-by-default data eliminates shared mutable state as a concern — concurrent tasks cannot corrupt each other's data. Reference counting with `Rc` is the main obstacle: `Rc` is not thread-safe. Migrating to `Arc` (atomic reference counting) is a mechanical change — `Arc` is a drop-in replacement with a small performance cost — but it touches every value allocation, so it is easier to do before the codebase is large.

The practical constraint: do not introduce thread-local global state, mutable singletons, or implicit shared context in the interpreter's Rust code. Keep the evaluator's state contained in explicit structures that could be duplicated or isolated per-task.

When concurrency is eventually implemented, the most natural model is message-passing (Erlang-style lightweight processes or Go-style goroutines with channels), which aligns with the functional, immutable-data philosophy. Each concurrent task would have its own evaluator state, communicating by sending immutable values through channels rather than sharing memory.

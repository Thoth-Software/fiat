# Fiat Benchmark Programs

Programs in ascending order of complexity, each level exercising a distinct capability the implementation needs to get right.

Unless a level says otherwise, benchmark files may assume the automatic Fiat prelude is available for list and helper functions such as `map`, `filter`, `fold`, `length`, `reverse`, `append`, `sort`, `not`, `>=`, `<=`, `max`, and `min`.

`(fiat Lux)` is required for namespaced standard-library modules such as `Map`, `Vector`, `String`, `Int`, `Float`, and `Math`.

Level 0 and Level 6 intentionally disable Lux and the automatic prelude. Every helper function used in those levels must be defined in the file.

---

## Level 0 — Primitives Only

No Lux, no imports, no automatic prelude. Tests: evaluator, the eight primitives, arithmetic, basic recursion.

### 0a. List construction and decomposition round-trip

Validates: `bind`, `first`, `rest`, `is?`, the identity law.

```lisp
(fiat roundtrip? (lst)
  (choose
    ((atom? lst) true)
    ((is? (first (bind 'x lst)) 'x) true)
    (true false)))

(roundtrip? '(a b c))   ;; → true
```

### 0b. Reverse a list using only primitives

Validates: tail recursion, `bind` as sole constructor, `atom?` as base case.

```lisp
(fiat reverse (lst)
  (fiat go (remaining acc)
    (choose
      ((atom? remaining) acc)
      (true (go (rest remaining) (bind (first remaining) acc)))))
  (go lst ()))

(reverse '(1 2 3 4 5))   ;; → (5 4 3 2 1)
```

### 0c. Zip two lists into a list of pairs

Validates: nested `bind`, parallel recursion over two lists.

```lisp
(fiat zip (xs ys)
  (choose
    ((atom? xs) ())
    ((atom? ys) ())
    (true (bind (bind (first xs) (bind (first ys) ()))
                (zip (rest xs) (rest ys))))))

(zip '(a b c) '(1 2 3))   ;; → ((a 1) (b 2) (c 3))
```

### 0d. Flatten a nested list structure

Validates: recursive descent into sublists, `atom?` distinguishing leaves, and safe primitive interrogation. `is?` is only called after `atom?` has established that the value is atomic.

```lisp
;; append itself is primitives-only
(fiat append (xs ys)
  (choose
    ((atom? xs) ys)
    (true (bind (first xs) (append (rest xs) ys)))))

(fiat flatten (lst)
  (choose
    ((atom? lst)
      (choose
        ((is? lst ()) ())
        (true (bind lst ()))))
    (true
      (append (flatten (first lst))
              (flatten (rest lst))))))

(flatten '(1 (2 (3 4) 5) (6)))   ;; → (1 2 3 4 5 6)
```

---

## Level 1 — Higher-Order Functions and Closures

Tests: lexical scoping, functions as values, closure capture.

### 1a. Compose two functions into one

Validates: returning a closure that captures `f` and `g`.

```lisp
(fiat compose (f g)
  (fiat () (x) (f (g x))))

(let ((inc-then-double (compose (fiat () (x) (* x 2))
                                (fiat () (x) (+ x 1)))))
  (inc-then-double 4))   ;; → 10  (4+1=5, 5*2=10)
```

### 1b. Currying — partial application by returning closures

Validates: nested closure capture, lexical rather than dynamic binding.

```lisp
(fiat add (a)
  (fiat () (b) (+ a b)))

(let ((add5 (add 5)))
  (map add5 '(1 2 3 4)))   ;; → (6 7 8 9)
```

### 1c. Church booleans — proving functions-as-values work

Validates: higher-order application, closures as data.

```lisp
(fiat T (a b) a)
(fiat F (a b) b)
(fiat church-not (p) (p F T))
(fiat church-and (p q) (p q F))
(fiat church-or (p q) (p T q))

(church-not T)             ;; → F  (the function object)
((church-and T T) 'y 'n)   ;; → y
((church-or F T) 'y 'n)    ;; → y
((church-or F F) 'y 'n)    ;; → n
```

### 1d. Accumulator factory — independent counters via closures

Validates: closures over recursive state. Pure functional style: returns a proper two-element list `(value next-counter)` because `bind` rejects non-list tails.

```lisp
(fiat make-counter (start)
  (fiat () ()
    (bind start (bind (make-counter (+ start 1)) ()))))

(let ((c (make-counter 0)))
  (let ((r1 (c)))                  ;; r1 = (0 <next>)
    (let ((r2 ((first (rest r1))))) ;; call next counter
      (first r2))))                ;; → 1
```

---

## Level 2 — Collection Types in Concert

Tests: vectors, maps, sets interoperating; structural construction.

### 2a. Build a frequency map from a list

Validates: `Map/get` with default, `Map/put`, `fold` over a list.
`Map/entries` returns a list of two-element lists, where each entry has the shape `(key value)`. This representation is used by later benchmark levels for result-tag inspection and report formatting.

```lisp
(fiat Lux)

(fiat frequencies (lst)
  (fold
    (fiat () (counts item)
      (Map/put counts item (+ 1 (Map/get counts item 0))))
    {}
    lst))

(frequencies '(a b a c b a))   ;; → {a 3 b 2 c 1}
```

### 2b. Group-by: partition a list into a map of lists by key function

Validates: higher-order + map construction, nested data.

```lisp
(fiat group-by (key-fn lst)
  (fold
    (fiat () (groups item)
      (let ((k (key-fn item))
            (existing (Map/get groups k ())))
        (Map/put groups k (bind item existing))))
    {}
    lst))

(group-by
  (fiat () (n) (choose ((= (% n 2) 0) :even) (true :odd)))
  '(1 2 3 4 5 6 7 8))
;; → {:odd (7 5 3 1) :even (8 6 4 2)}
```

### 2c. Set-driven filtering — keep only items in the allowed set

Validates: sets as lookup tables, `has?` in a predicate, interop with `filter`.

```lisp
(fiat allow-only (allowed lst)
  (filter (fiat () (x) (has? x allowed)) lst))

(allow-only #{:a :c :e} '(:a :b :c :d :e :f))   ;; → (:a :c :e)
```

### 2d. Index a vector of records by a key field

Validates: `Vector/to-list` at the vector/list boundary, `Map` construction from vector data.

```lisp
(let ((people [{:name "Alice" :dept :eng}
               {:name "Bob" :dept :design}
               {:name "Carol" :dept :eng}]))
  (group-by (fiat () (p) (Map/get p :dept ())) (Vector/to-list people)))
;; → {:eng ({:name "Carol" :dept :eng} {:name "Alice" :dept :eng})
;;    :design ({:name "Bob" :dept :design})}
```

---

## Level 3 — String Processing Pipelines

Tests: String module, threading macros, `as-codepoints`.

### 3a. Slugify a title

Validates: thread-first chaining, String module functions.

```lisp
(fiat Lux)

(fiat slugify (title)
  (-> title
      String/downcase
      String/trim
      (String/replace " " "-")
      (String/replace "'" "")))

(slugify " Hello World's End ")   ;; → "hello-worlds-end"
```

### 3b. Palindrome check via codepoint decomposition

Validates: `as-codepoints`, `reverse`, `is?` on integers. For inline numeric values, identity and value equality coincide.

```lisp
(fiat lists-equal? (xs ys)
  (choose
    ((atom? xs) (atom? ys))
    ((atom? ys) false)
    ((not (is? (first xs) (first ys))) false)
    (true (lists-equal? (rest xs) (rest ys)))))

(fiat palindrome? (s)
  (let ((cps (-> s String/downcase String/trim as-codepoints)))
    (lists-equal? cps (reverse cps))))

(palindrome? "racecar")   ;; → true
(palindrome? "hello")     ;; → false
```

### 3c. Extract all words longer than n characters

Validates: thread-last, `filter` with closure, `String/length`.

```lisp
(fiat long-words (text min-len)
  (->> (String/split text " ")
       (filter (fiat () (w) (> (String/length w) min-len)))))

(long-words "the quick brown fox jumped" 4)   ;; → ("quick" "brown" "jumped")
```

### 3d. Caesar cipher via codepoint manipulation

Validates: `as-codepoints`, `map` over codepoints, arithmetic on characters.

```lisp
(fiat caesar-encrypt (text shift)
  (let ((encrypt-char
          (fiat () (cp)
            (choose
              ;; lowercase a-z: codepoints 97-122
              ((and (>= cp 97) (<= cp 122))
                (+ 97 (% (+ (- cp 97) shift) 26)))
              ;; uppercase A-Z: codepoints 65-90
              ((and (>= cp 65) (<= cp 90))
                (+ 65 (% (+ (- cp 65) shift) 26)))
              (true cp)))))
    (-> text
        as-codepoints
        (map encrypt-char)
        from-codepoints)))

(caesar-encrypt "Hello" 3)                     ;; → "Khoor"
(caesar-encrypt (caesar-encrypt "Hello" 3) 23) ;; → "Hello"
```

---

## Level 4 — Result Types and Error Pipelines

Tests: result type handling, `choose`-based dispatch, error threading.

### Convention

`{:ok value}` represents success.

`{:err reason}` represents failure.

A result map contains exactly one entry. Result dispatch inspects the entry key, not the payload, because the payload may be a collection and `is?` is only valid on atoms.

```lisp
(fiat Lux)

(fiat ok (v) {:ok v})
(fiat err (reason) {:err reason})

;; A result map contains exactly one key-value pair.
;; Map/entries returns a list of entries.
;; Each entry is a two-element list: (key value).
(fiat result-entry (result)
  (first (Map/entries result)))

(fiat result-tag (result)
  (first (result-entry result)))

(fiat result-value (result)
  (first (rest (result-entry result))))

(fiat ok? (result)
  (is? (result-tag result) :ok))

(fiat err? (result)
  (is? (result-tag result) :err))
```

### 4a. Safe division

```lisp
(fiat safe-div (a b)
  (choose
    ((= b 0) (err "division by zero"))
    (true    (ok (/ a b)))))

(safe-div 10 3)   ;; → {:ok 3}
(safe-div 10 0)   ;; → {:err "division by zero"}
```

### 4b. Chain results — short-circuit on first error

Validates: higher-order result threading, closure over continuation.

```lisp
(fiat then (result f)
  (choose
    ((ok? result) (f (result-value result)))
    (true         result)))

;; Parse a config: extract required fields, fail on any missing.
;; This deliberately returns {:ok <map>} on success, proving that
;; ok? does not inspect the payload with is?.
(fiat parse-config (raw)
  (-> (ok raw)
      (then (fiat () (cfg)
        (choose
          ((Map/get cfg :host false) (ok cfg))
          (true (err "missing :host")))))
      (then (fiat () (cfg)
        (choose
          ((Map/get cfg :port false) (ok cfg))
          (true (err "missing :port")))))))

(parse-config {:host "localhost" :port 8080})
;; → {:ok {:host "localhost" :port 8080}}

(parse-config {:host "localhost"})
;; → {:err "missing :port"}

(parse-config {})
;; → {:err "missing :host"}
```

---

## Level 5 — Mutual Tail Recursion / State Machine

Tests: TCO across mutually recursive functions, game-style state logic.

A simple NPC dialogue state machine. Each state is a function that tail-calls the next state. Without proper TCO, this would overflow on any non-trivial dialogue.

```lisp
(fiat Lux)

(fiat dialogue (state context)
  (choose
    ((is? state :greeting)  (state-greeting context))
    ((is? state :question)  (state-question context))
    ((is? state :farewell)  (state-farewell context))
    (true                   (state-farewell context))))

(fiat state-greeting (context)
  (let ((response (Map/put context :said "Hello, traveler!")))
    (choose
      ((has? :has-quest (Map/get context :flags #{}))
        (dialogue :question response))
      (true
        (dialogue :farewell response)))))

(fiat state-question (context)
  (let ((response (Map/put context :said "Have you found the amulet?")))
    (choose
      ((has? :has-amulet (Map/get context :flags #{}))
        (dialogue :farewell (Map/put response :quest-complete true)))
      (true
        (dialogue :farewell response)))))

(fiat state-farewell (context)
  (Map/put context :said "Safe travels."))

;; Run it
(dialogue :greeting {:flags #{:has-quest :has-amulet}})
;; → {:flags #{:has-quest :has-amulet}
;;    :said "Safe travels."
;;    :quest-complete true}
```

---

## Level 6 — Deriving Combinators (Self-Hosting Stress Test)

Tests: that the primitives are sufficient for real abstraction. All of these use only the eight primitives + arithmetic after desugaring. `let` syntax is permitted because it expands to anonymous `fiat` application. No Lux import or automatic prelude is allowed. This is the bootstrap layer. Every helper function used must be defined in this file.

### Prerequisites — helpers derived from primitives

These definitions must precede the combinators that depend on them.

```lisp
;; not — boolean negation
(fiat not (x)
  (choose
    (x false)
    (true true)))

;; reverse — from Level 0, needed by partition
(fiat reverse (lst)
  (fiat go (remaining acc)
    (choose
      ((atom? remaining) acc)
      (true (go (rest remaining) (bind (first remaining) acc)))))
  (go lst ()))

;; length — needed by sort
(fiat length (lst)
  (fiat go (remaining acc)
    (choose
      ((atom? remaining) acc)
      (true (go (rest remaining) (+ acc 1)))))
  (go lst 0))

;; odd? — needed by partition example
(fiat odd? (n) (not (= (% n 2) 0)))
```

### Combinators

```lisp
;; take — first n elements
(fiat take (n lst)
  (choose
    ((= n 0)     ())
    ((atom? lst) ())
    (true (bind (first lst) (take (- n 1) (rest lst))))))

;; drop — skip first n elements
(fiat drop (n lst)
  (choose
    ((= n 0)     lst)
    ((atom? lst) ())
    (true (drop (- n 1) (rest lst)))))

;; nth — zero-indexed access
(fiat nth (n lst)
  (choose
    ((atom? lst) ())
    ((= n 0)     (first lst))
    (true        (nth (- n 1) (rest lst)))))

;; zip-with — generalized zip
(fiat zip-with (f xs ys)
  (choose
    ((atom? xs) ())
    ((atom? ys) ())
    (true (bind (f (first xs) (first ys))
                (zip-with f (rest xs) (rest ys))))))

;; any? — does any element satisfy the predicate?
(fiat any? (pred lst)
  (choose
    ((atom? lst) false)
    ((pred (first lst)) true)
    (true (any? pred (rest lst)))))

;; all? — do all elements satisfy the predicate?
(fiat all? (pred lst)
  (choose
    ((atom? lst) true)
    ((not (pred (first lst))) false)
    (true (all? pred (rest lst)))))

;; partition — split into (satisfying not-satisfying)
(fiat partition (pred lst)
  (fiat go (remaining yes no)
    (choose
      ((atom? remaining) (bind (reverse yes) (bind (reverse no) ())))
      ((pred (first remaining))
        (go (rest remaining) (bind (first remaining) yes) no))
      (true
        (go (rest remaining) yes (bind (first remaining) no)))))
  (go lst () ()))

(partition odd? '(1 2 3 4 5 6))
;; → ((1 3 5) (2 4 6))

;; sort — mergesort using only primitives
(fiat sort (compare lst)
  (fiat merge (xs ys)
    (choose
      ((atom? xs) ys)
      ((atom? ys) xs)
      ((compare (first xs) (first ys))
        (bind (first xs) (merge (rest xs) ys)))
      (true
        (bind (first ys) (merge xs (rest ys))))))
  (let ((len (length lst)))
    (choose
      ((< len 2) lst)
      (true
        (let ((mid (/ len 2)))
          (merge (sort compare (take mid lst))
                 (sort compare (drop mid lst))))))))

(sort < '(5 3 8 1 9 2 7))   ;; → (1 2 3 5 7 8 9)
```

---

## Level 7 — Orpheus Game Entity System

Tests: maps as records, immutable state transformation, set membership, threading, closures — the full integration of all features. All uses of `and` are binary, matching the desugar rule `(and a b)` → `(choose (a b) (true false))`.

```lisp
(fiat Lux)

;; --- Entity constructors ---

(fiat make-entity (id kind props)
  (-> {:id id :kind kind :alive true :tags #{}}
      (Map/merge props)))

(fiat make-player (id name)
  (make-entity id :player
    {:name name :hp 100 :max-hp 100 :mp 50 :pos {:x 0.0 :y 0.0}
     :inventory [] :tags #{:controllable :collidable}}))

(fiat make-enemy (id name hp damage)
  (make-entity id :enemy
    {:name name :hp hp :max-hp hp :damage damage
     :pos {:x 0.0 :y 0.0} :tags #{:hostile :collidable}}))

(fiat make-item (id name effect)
  (make-entity id :item
    {:name name :effect effect :tags #{:pickupable}}))


;; --- Pure state transforms ---

(fiat damage-entity (entity amount)
  (let ((new-hp (max 0 (- (Map/get entity :hp 0) amount))))
    (-> entity
        (Map/put :hp new-hp)
        (Map/put :alive (> new-hp 0)))))

(fiat heal-entity (entity amount)
  (let ((max-hp (Map/get entity :max-hp 100))
        (new-hp (min max-hp (+ (Map/get entity :hp 0) amount))))
    (Map/put entity :hp new-hp)))

(fiat move-entity (entity dx dy)
  (let ((pos (Map/get entity :pos {:x 0.0 :y 0.0})))
    (Map/put entity :pos
      {:x (+ (Map/get pos :x 0.0) dx)
       :y (+ (Map/get pos :y 0.0) dy)})))

(fiat add-to-inventory (player item)
  (Map/put player :inventory
    (Vector/append (Map/get player :inventory []) item)))


;; --- Collision detection (distance-based) ---

(fiat distance (e1 e2)
  (let ((p1 (Map/get e1 :pos {:x 0.0 :y 0.0}))
        (p2 (Map/get e2 :pos {:x 0.0 :y 0.0}))
        (dx (- (Map/get p1 :x 0.0) (Map/get p2 :x 0.0)))
        (dy (- (Map/get p1 :y 0.0) (Map/get p2 :y 0.0))))
    (Math/sqrt (+ (* dx dx) (* dy dy)))))

(fiat colliding? (e1 e2 radius)
  (and (has? :collidable (Map/get e1 :tags #{}))
       (and (has? :collidable (Map/get e2 :tags #{}))
            (< (distance e1 e2) radius))))


;; --- Per-tick update: process all entities against the world ---

(fiat find-collisions (entity others radius)
  (filter (fiat () (other) (colliding? entity other radius)) others))

(fiat apply-combat (player enemies)
  (fold
    (fiat () (state enemy)
      (choose
        ((not (Map/get enemy :alive true)) state)
        (true
          (let ((dmg (Map/get enemy :damage 10)))
            {:player (damage-entity (Map/get state :player {}) dmg)
             :log (bind (String/concat ["Hit by " (Map/get enemy :name "?")
                                        " for " (Int/to-string dmg) " damage!"])
                        (Map/get state :log ()))}))))
    {:player player :log ()}
    enemies))

(fiat tick (world)
  (let ((player (Map/get world :player {}))
        (enemies (->> (Map/get world :entities [])
                      Vector/to-list
                      (filter (fiat () (e)
                        (and (is? (Map/get e :kind ()) :enemy)
                             (Map/get e :alive true))))))
        (nearby (find-collisions player enemies 1.5))
        (result (apply-combat player nearby)))
    (-> world
        (Map/put :player (Map/get result :player {}))
        (Map/put :log (Map/get result :log ())))))


;; --- Build a world and simulate one tick ---

(let ((world
        {:player (-> (make-player :p1 "Hero")
                     (move-entity 5.0 3.0))
         :entities [(-> (make-enemy :e1 "Goblin" 30 5)
                        (move-entity 5.5 3.2))
                    (-> (make-enemy :e2 "Skeleton" 50 8)
                        (move-entity 100.0 100.0))
                    (make-item :i1 "Health Potion" {:type :heal :amount 25})]}))
  (tick world))
;; The goblin is within 1.5 units → combat. The skeleton is far away → ignored.
;; Result: player takes 5 damage (hp: 95), log has the hit message.
```

---

## Level 8 — Scripting: File Processing Pipeline

Tests: Firmamentum bindings, result-type discipline for I/O, end-to-end scripting. Integrates Level 4's result conventions with real file I/O and parsing. `Fs/read`, `Fs/write`, and `Int/parse` all return result values, and the pipeline must handle errors explicitly.

Read a CSV, compute per-department salary stats, write a report.

```lisp
(fiat Lux)
(fiat Firmamentum)

;; --- Result helpers ---

(fiat ok (v) {:ok v})
(fiat err (reason) {:err reason})

;; A result map contains exactly one key-value pair.
;; Map/entries returns a list of entries.
;; Each entry is a two-element list: (key value).
(fiat result-entry (result)
  (first (Map/entries result)))

(fiat result-tag (result)
  (first (result-entry result)))

(fiat result-value (result)
  (first (rest (result-entry result))))

(fiat ok? (result)
  (is? (result-tag result) :ok))

(fiat err? (result)
  (is? (result-tag result) :err))

(fiat then (result f)
  (choose
    ((ok? result) (f (result-value result)))
    (true         result)))


;; --- Result-aware list traversal ---

;; traverse maps a result-returning function over a list.
;; It short-circuits on the first error.
;;
;; Example shape:
;;   (traverse parse-csv-line lines)
;;   ;; → {:ok (<record> <record> ...)}
;;   ;; or {:err reason}
(fiat traverse (f lst)
  (choose
    ((atom? lst) (ok ()))
    (true
      (then (f (first lst))
        (fiat () (head)
          (then (traverse f (rest lst))
            (fiat () (tail)
              (ok (bind head tail)))))))))


;; --- Local helpers ---

;; group-by is defined locally rather than assumed from the prelude.
(fiat group-by (key-fn lst)
  (fold
    (fiat () (groups item)
      (let ((k (key-fn item))
            (existing (Map/get groups k ())))
        (Map/put groups k (bind item existing))))
    {}
    lst))


;; --- CSV parsing ---

;; Int/parse returns a result, so parse-csv-line also returns a result.
(fiat parse-csv-line (line)
  (let ((fields (String/split line ",")))
    (let ((name (first fields))
          (dept (first (rest fields)))
          (salary-str (first (rest (rest fields)))))
      (then (Int/parse (String/trim salary-str))
        (fiat () (salary)
          (ok {:name name
               :dept dept
               :salary salary}))))))


;; --- Statistics ---

(fiat compute-stats (records)
  (->> records
       (group-by (fiat () (r) (Map/get r :dept "")))
       (Map/map-values
         (fiat () (dept-records)
           (let ((salaries (map (fiat () (r) (Map/get r :salary 0)) dept-records))
                 (total (fold + 0 salaries))
                 (count (length salaries)))
             {:count count
              :total total
              :avg (/ total count)
              :max (fold max 0 salaries)
              :min (fold min 999999999 salaries)})))))


;; --- Report formatting ---

(fiat format-report (stats)
  (->> (Map/entries stats)
       (sort
         (fiat () (a b)
           (< (Map/get (first (rest a)) :avg 0)
              (Map/get (first (rest b)) :avg 0))))
       (map
         (fiat () (entry)
           (let ((dept (first entry))
                 (s (first (rest entry))))
             (String/concat
               [dept ": "
                (Int/to-string (Map/get s :count 0)) " employees, "
                "avg $" (Int/to-string (Map/get s :avg 0)) ", "
                "range $" (Int/to-string (Map/get s :min 0))
                "-$" (Int/to-string (Map/get s :max 0))]))))
       (String/join "\n")))


;; --- End-to-end file pipeline ---

(-> (Fs/read "employees.csv")
    (then
      (fiat () (contents)
        (let ((lines (-> contents
                         String/trim
                         (String/split "\n")
                         rest)))
          (traverse parse-csv-line lines))))
    (then
      (fiat () (records)
        (ok (-> records
                compute-stats
                format-report))))
    (then
      (fiat () (report)
        (Fs/write report "report.txt"))))

;; → {:ok true} on success
;; → {:err reason} on read, parse, or write failure
```

---

## What Each Level Tests

| Level | Focus                  | Key Implementation Requirement                                                      |
| ----- | ---------------------- | ----------------------------------------------------------------------------------- |
| 0     | Primitives only        | Evaluator, `bind`/`first`/`rest`, `atom?`, basic recursion                          |
| 1     | Higher-order functions | Lexical scoping, closures, functions as values                                      |
| 2     | Collections            | Persistent vectors/maps/sets, explicit `Vector/to-list`, list-only `fold`           |
| 3     | Strings                | Unicode decomposition via codepoints, threading macros, `String` module             |
| 4     | Error handling         | Maps-as-result-types, tag-based result dispatch, higher-order chaining              |
| 5     | State machines         | Mutual tail recursion, TCO across multiple functions                                |
| 6     | Self-hosting           | Primitives suffice for mergesort, partition, combinators                            |
| 7     | Game entities          | Full integration: immutable maps, sets, closures, threading                         |
| 8     | Scripting I/O          | Firmamentum bindings, result-type discipline for I/O, file read → transform → write |

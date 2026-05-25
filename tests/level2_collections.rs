use fiat::eval::eval_program;
use fiat::prelude;
use fiat::reader::read;
use fiat::value::{InternedSymbol, Value};

fn eval_source(source: &str) -> Value {
    let forms = read(source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    eval_program(&forms, &env).expect("eval error")
}

fn kw(name: &str) -> Value {
    Value::Keyword(InternedSymbol::new(name))
}

fn sym(name: &str) -> Value {
    Value::Symbol(InternedSymbol::new(name))
}

// --- 2a. Frequency map ---

#[test]
fn level2a_frequencies() {
    let source = r#"
        (fiat Lux)

        (fiat frequencies (lst)
          (fold
            (fiat () (counts item)
              (Map/put counts item (+ 1 (Map/get counts item 0))))
            {}
            lst))

        (frequencies '(a b a c b a))
    "#;
    let result = eval_source(source);
    // Result is a map — compare structurally
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 3);
        assert_eq!(m.get(&sym("a")), Some(&Value::Int(3)));
        assert_eq!(m.get(&sym("b")), Some(&Value::Int(2)));
        assert_eq!(m.get(&sym("c")), Some(&Value::Int(1)));
    } else {
        panic!("expected map, got {result}");
    }
}

// --- 2b. Group-by even/odd ---

#[test]
fn level2b_group_by() {
    let source = r#"
        (fiat Lux)

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
    "#;
    let result = eval_source(source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 2);
        let odd = m.get(&kw("odd")).expect(":odd key missing");
        let even = m.get(&kw("even")).expect(":even key missing");
        // fold+bind builds in reverse: (7 5 3 1) and (8 6 4 2)
        assert_eq!(odd.to_string(), "(7 5 3 1)");
        assert_eq!(even.to_string(), "(8 6 4 2)");
    } else {
        panic!("expected map, got {result}");
    }
}

// --- 2c. Set-driven filtering ---

#[test]
fn level2c_set_filtering() {
    let source = r#"
        (fiat Lux)

        (fiat allow-only (allowed lst)
          (filter (fiat () (x) (has? x allowed)) lst))

        (allow-only #{:a :c :e} '(:a :b :c :d :e :f))
    "#;
    let result = eval_source(source);
    // filter preserves order, so result is a cons-list
    assert_eq!(result.to_string(), "(:a :c :e)");
}

// --- 2d. Index vector records by key ---

#[test]
fn level2d_index_records() {
    let source = r#"
        (fiat Lux)

        (fiat group-by (key-fn lst)
          (fold
            (fiat () (groups item)
              (let ((k (key-fn item))
                    (existing (Map/get groups k ())))
                (Map/put groups k (bind item existing))))
            {}
            lst))

        (let ((people [{:name "Alice" :dept :eng}
                       {:name "Bob" :dept :design}
                       {:name "Carol" :dept :eng}]))
          (group-by (fiat () (p) (Map/get p :dept ())) (Vector/to-list people)))
    "#;
    let result = eval_source(source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 2);

        let eng = m.get(&kw("eng")).expect(":eng key missing");
        let design = m.get(&kw("design")).expect(":design key missing");

        // fold+bind builds in reverse order of traversal
        // eng should have Carol then Alice (reversed)
        let eng_str = eng.to_string();
        assert!(eng_str.contains("Carol"), "eng missing Carol: {eng_str}");
        assert!(eng_str.contains("Alice"), "eng missing Alice: {eng_str}");

        let design_str = design.to_string();
        assert!(
            design_str.contains("Bob"),
            "design missing Bob: {design_str}"
        );
    } else {
        panic!("expected map, got {result}");
    }
}

// --- Full file ---

#[test]
fn level2_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level2.fiat").expect("could not read level2.fiat");
    let forms = read(&source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    let result = eval_program(&forms, &env).expect("eval error");
    // The final form is 2d, whose value is a map with :eng and :design keys
    if let Value::Map(m) = &result {
        assert!(m.contains_key(&kw("eng")));
        assert!(m.contains_key(&kw("design")));
    } else {
        panic!("expected map, got {result}");
    }
}

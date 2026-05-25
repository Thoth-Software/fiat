use std::rc::Rc;

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

const RESULT_HELPERS: &str = r#"
    (fiat Lux)

    (fiat ok (v) {:ok v})
    (fiat err (reason) {:err reason})

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
"#;

const SAFE_DIV: &str = r#"
    (fiat safe-div (a b)
      (choose
        ((= b 0) (err "division by zero"))
        (true    (ok (/ a b)))))
"#;

const THEN_AND_PARSE: &str = r#"
    (fiat then (result f)
      (choose
        ((ok? result) (f (result-value result)))
        (true         result)))

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
"#;

// --- 4a. Safe division ---

#[test]
fn level4a_safe_div_ok() {
    let source = format!("{RESULT_HELPERS}{SAFE_DIV}(safe-div 10 3)");
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 1);
        assert_eq!(m.get(&kw("ok")), Some(&Value::Int(3)));
    } else {
        panic!("expected map, got {result}");
    }
}

#[test]
fn level4a_safe_div_err() {
    let source = format!("{RESULT_HELPERS}{SAFE_DIV}(safe-div 10 0)");
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.get(&kw("err")),
            Some(&Value::String(Rc::from("division by zero")))
        );
    } else {
        panic!("expected map, got {result}");
    }
}

// --- 4b. Parse config ---

#[test]
fn level4b_parse_config_ok() {
    let source = format!(
        r#"{RESULT_HELPERS}{SAFE_DIV}{THEN_AND_PARSE}
        (parse-config {{:host "localhost" :port 8080}})
        "#
    );
    let result = eval_source(&source);
    if let Value::Map(outer) = &result {
        assert_eq!(outer.len(), 1);
        let payload = outer.get(&kw("ok")).expect(":ok key missing");
        if let Value::Map(inner) = payload {
            assert_eq!(inner.len(), 2);
            assert_eq!(
                inner.get(&kw("host")),
                Some(&Value::String(Rc::from("localhost")))
            );
            assert_eq!(inner.get(&kw("port")), Some(&Value::Int(8080)));
        } else {
            panic!("expected inner map, got {payload}");
        }
    } else {
        panic!("expected map, got {result}");
    }
}

#[test]
fn level4b_parse_config_missing_port() {
    let source = format!(
        r#"{RESULT_HELPERS}{SAFE_DIV}{THEN_AND_PARSE}
        (parse-config {{:host "localhost"}})
        "#
    );
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.get(&kw("err")),
            Some(&Value::String(Rc::from("missing :port")))
        );
    } else {
        panic!("expected map, got {result}");
    }
}

#[test]
fn level4b_parse_config_missing_host() {
    let source = format!(
        "{RESULT_HELPERS}{SAFE_DIV}{THEN_AND_PARSE}
        (parse-config {{}})"
    );
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.get(&kw("err")),
            Some(&Value::String(Rc::from("missing :host")))
        );
    } else {
        panic!("expected map, got {result}");
    }
}

// --- Full file ---

#[test]
fn level4_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level4.fiat").expect("could not read level4.fiat");
    let forms = read(&source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    let result = eval_program(&forms, &env).expect("eval error");
    if let Value::Map(outer) = &result {
        assert_eq!(outer.len(), 1);
        let payload = outer.get(&kw("ok")).expect(":ok key missing");
        if let Value::Map(inner) = payload {
            assert_eq!(
                inner.get(&kw("host")),
                Some(&Value::String(Rc::from("localhost")))
            );
            assert_eq!(inner.get(&kw("port")), Some(&Value::Int(8080)));
        } else {
            panic!("expected inner map, got {payload}");
        }
    } else {
        panic!("expected map, got {result}");
    }
}

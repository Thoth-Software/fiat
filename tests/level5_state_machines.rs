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

const DIALOGUE: &str = r#"
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
"#;

#[test]
fn level5_full_dialogue() {
    let source = format!(
        "{DIALOGUE}
        (dialogue :greeting {{:flags #{{:has-quest :has-amulet}}}})"
    );
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(m.len(), 3);
        assert_eq!(
            m.get(&kw("said")),
            Some(&Value::String(Rc::from("Safe travels.")))
        );
        assert_eq!(m.get(&kw("quest-complete")), Some(&Value::Bool(true)));
        let flags = m.get(&kw("flags")).expect(":flags missing");
        if let Value::Set(s) = flags {
            assert_eq!(s.len(), 2);
            assert!(s.contains(&kw("has-quest")));
            assert!(s.contains(&kw("has-amulet")));
        } else {
            panic!("expected set for :flags, got {flags}");
        }
    } else {
        panic!("expected map, got {result}");
    }
}

#[test]
fn level5_no_quest_short_circuits() {
    // Without :has-quest, greeting goes straight to farewell, no quest-complete.
    let source = format!(
        "{DIALOGUE}
        (dialogue :greeting {{:flags #{{}}}})"
    );
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(
            m.get(&kw("said")),
            Some(&Value::String(Rc::from("Safe travels.")))
        );
        assert_eq!(m.get(&kw("quest-complete")), None);
    } else {
        panic!("expected map, got {result}");
    }
}

#[test]
fn level5_quest_without_amulet() {
    // Has quest but no amulet: question state runs, no quest-complete set.
    let source = format!(
        "{DIALOGUE}
        (dialogue :greeting {{:flags #{{:has-quest}}}})"
    );
    let result = eval_source(&source);
    if let Value::Map(m) = &result {
        assert_eq!(
            m.get(&kw("said")),
            Some(&Value::String(Rc::from("Safe travels.")))
        );
        assert_eq!(m.get(&kw("quest-complete")), None);
    } else {
        panic!("expected map, got {result}");
    }
}

#[test]
fn level5_deep_transitions_tco() {
    // A synthetic mutual-recursion chain that ping-pongs deeply between two
    // states; without TCO this would overflow the stack.
    let source = r#"
        (fiat ping (n)
          (choose
            ((= n 0) :done)
            (true (pong (- n 1)))))
        (fiat pong (n)
          (choose
            ((= n 0) :done)
            (true (ping (- n 1)))))
        (ping 200000)
    "#;
    let result = eval_source(source);
    assert_eq!(result, kw("done"));
}

#[test]
fn level5_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level5.fiat").expect("could not read level5.fiat");
    let forms = read(&source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    let result = eval_program(&forms, &env).expect("eval error");
    if let Value::Map(m) = &result {
        assert_eq!(
            m.get(&kw("said")),
            Some(&Value::String(Rc::from("Safe travels.")))
        );
        assert_eq!(m.get(&kw("quest-complete")), Some(&Value::Bool(true)));
        assert!(m.contains_key(&kw("flags")));
    } else {
        panic!("expected map, got {result}");
    }
}

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

fn get_field<'a>(map: &'a im_rc::HashMap<Value, Value>, key: &str) -> &'a Value {
    map.get(&kw(key))
        .unwrap_or_else(|| panic!("missing field :{key}"))
}

#[test]
fn level7_tick_player_hp() {
    let source =
        std::fs::read_to_string("benchmarks/level7.fiat").expect("could not read level7.fiat");
    let result = eval_source(&source);
    let Value::Map(world) = &result else {
        panic!("expected map, got {result}");
    };
    let Value::Map(player) = get_field(world, "player") else {
        panic!("expected map for :player");
    };
    assert_eq!(player.get(&kw("hp")), Some(&Value::Int(95)));
}

#[test]
fn level7_tick_player_alive() {
    let source =
        std::fs::read_to_string("benchmarks/level7.fiat").expect("could not read level7.fiat");
    let result = eval_source(&source);
    let Value::Map(world) = &result else {
        panic!("expected map, got {result}");
    };
    let Value::Map(player) = get_field(world, "player") else {
        panic!("expected map for :player");
    };
    assert_eq!(player.get(&kw("alive")), Some(&Value::Bool(true)));
}

#[test]
fn level7_tick_log_contains_goblin_hit() {
    let source =
        std::fs::read_to_string("benchmarks/level7.fiat").expect("could not read level7.fiat");
    let result = eval_source(&source);
    let Value::Map(world) = &result else {
        panic!("expected map, got {result}");
    };
    let log = get_field(world, "log");
    let expected_msg = Value::String(Rc::from("Hit by Goblin for 5 damage!"));
    let mut found = false;
    let mut current = log;
    loop {
        match current {
            Value::List(cons) => {
                if cons.head == expected_msg {
                    found = true;
                    break;
                }
                current = &cons.tail;
            }
            Value::Nil => break,
            _ => break,
        }
    }
    assert!(
        found,
        "expected log to contain goblin hit message, got {log}"
    );
}

#[test]
fn level7_skeleton_not_in_log() {
    let source =
        std::fs::read_to_string("benchmarks/level7.fiat").expect("could not read level7.fiat");
    let result = eval_source(&source);
    let Value::Map(world) = &result else {
        panic!("expected map, got {result}");
    };
    let log = get_field(world, "log");
    let skeleton_msg = Value::String(Rc::from("Hit by Skeleton for 8 damage!"));
    let mut current = log;
    loop {
        match current {
            Value::List(cons) => {
                assert_ne!(cons.head, skeleton_msg, "skeleton should not appear in log");
                current = &cons.tail;
            }
            _ => break,
        }
    }
}

#[test]
fn level7_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level7.fiat").expect("could not read level7.fiat");
    let forms = read(&source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    let result = eval_program(&forms, &env).expect("eval error");
    let Value::Map(world) = &result else {
        panic!("expected map, got {result}");
    };
    let Value::Map(player) = get_field(world, "player") else {
        panic!("expected map for :player");
    };
    assert_eq!(player.get(&kw("hp")), Some(&Value::Int(95)));
    assert_eq!(player.get(&kw("alive")), Some(&Value::Bool(true)));
    assert!(world.contains_key(&kw("log")));
}

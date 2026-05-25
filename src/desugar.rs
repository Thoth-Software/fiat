use std::cell::Cell;

use crate::value::{InternedSymbol, Value, list_from_vec, list_to_vec};

thread_local! {
    static OR_TMP_COUNTER: Cell<u64> = const { Cell::new(0) };
}

/// Desugar a sequence of top-level forms.
pub fn desugar_forms(forms: &[Value]) -> Vec<Value> {
    forms.iter().map(desugar).collect()
}

/// Recursively rewrite the bootstrap sugar (`let`, `and`, `or`) into the
/// primitive forms the evaluator understands. Anything inside a `behold`
/// form is inert data and is left untouched.
pub fn desugar(value: &Value) -> Value {
    match value {
        Value::List(_) => desugar_list(value),
        _ => value.clone(),
    }
}

fn desugar_list(value: &Value) -> Value {
    let items = list_to_vec(value);
    if let Some(head) = items.first().and_then(Value::as_symbol) {
        match head.name() {
            // Quoted data: return verbatim, do not descend.
            "behold" => return value.clone(),
            "let" if items.len() >= 2 => return desugar_let(&items),
            "and" if items.len() == 3 => return desugar_and(&items[1], &items[2]),
            "or" if items.len() == 3 => return desugar_or(&items[1], &items[2]),
            _ => {}
        }
    }
    list_from_vec(items.iter().map(desugar).collect())
}

/// `(let ((x v) (y w)) body...)` → `((fiat () (x) ((fiat () (y) body...) w)) v)`.
/// Sequential (`let*`) scoping: each binding value sees the earlier bindings.
fn desugar_let(items: &[Value]) -> Value {
    let bindings: Vec<(Value, Value)> = parse_bindings(&items[1])
        .into_iter()
        .map(|(name, val)| (name, desugar(&val)))
        .collect();
    let body: Vec<Value> = items[2..].iter().map(desugar).collect();
    build_let(&bindings, &body)
}

fn build_let(bindings: &[(Value, Value)], body: &[Value]) -> Value {
    match bindings.split_first() {
        // `(let () body...)` → `((fiat () () body...))`
        None => {
            let mut lambda = vec![sym("fiat"), Value::Nil, Value::Nil];
            lambda.extend(body.iter().cloned());
            list_from_vec(vec![list_from_vec(lambda)])
        }
        Some(((name, value), rest)) => {
            let inner_body = if rest.is_empty() {
                body.to_vec()
            } else {
                vec![build_let(rest, body)]
            };
            let mut lambda = vec![sym("fiat"), Value::Nil, list_from_vec(vec![name.clone()])];
            lambda.extend(inner_body);
            list_from_vec(vec![list_from_vec(lambda), value.clone()])
        }
    }
}

fn parse_bindings(value: &Value) -> Vec<(Value, Value)> {
    list_to_vec(value)
        .iter()
        .filter_map(|pair| {
            let parts = list_to_vec(pair);
            match parts.as_slice() {
                [name, val] => Some((name.clone(), val.clone())),
                _ => None,
            }
        })
        .collect()
}

/// `(and a b)` → `(choose (a b) (true false))`.
fn desugar_and(a: &Value, b: &Value) -> Value {
    list_from_vec(vec![
        sym("choose"),
        list_from_vec(vec![desugar(a), desugar(b)]),
        list_from_vec(vec![Value::Bool(true), Value::Bool(false)]),
    ])
}

/// `(or a b)` → `(let ((tmp a)) (choose (tmp tmp) (true b)))`, with a fresh
/// `tmp` so `a` is evaluated exactly once. The resulting `let` is desugared
/// in turn, which also desugars `a` and `b`.
fn desugar_or(a: &Value, b: &Value) -> Value {
    let tmp = fresh_or_tmp();
    let let_form = list_from_vec(vec![
        sym("let"),
        list_from_vec(vec![list_from_vec(vec![tmp.clone(), a.clone()])]),
        list_from_vec(vec![
            sym("choose"),
            list_from_vec(vec![tmp.clone(), tmp]),
            list_from_vec(vec![Value::Bool(true), b.clone()]),
        ]),
    ]);
    desugar(&let_form)
}

fn fresh_or_tmp() -> Value {
    let n = OR_TMP_COUNTER.with(|c| {
        let current = c.get();
        c.set(current + 1);
        current
    });
    Value::Symbol(InternedSymbol::new(&format!("__or_tmp_{n}")))
}

fn sym(name: &str) -> Value {
    Value::Symbol(InternedSymbol::new(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Env;
    use crate::eval::eval_program;
    use crate::reader::{read, read_one};

    fn desugared(source: &str) -> String {
        let form = read_one(source).expect("read error");
        desugar(&form).to_string()
    }

    fn run(source: &str) -> Value {
        let forms = read(source).expect("read error");
        let env = Env::new();
        eval_program(&forms, &env).expect("eval error")
    }

    #[test]
    fn let_single_binding() {
        assert_eq!(desugared("(let ((x 5)) x)"), "((fiat () (x) x) 5)");
    }

    #[test]
    fn let_sequential_bindings() {
        assert_eq!(
            desugared("(let ((x 1) (y (+ x 1))) (+ x y))"),
            "((fiat () (x) ((fiat () (y) (+ x y)) (+ x 1))) 1)"
        );
    }

    #[test]
    fn let_multi_form_body() {
        assert_eq!(desugared("(let ((x 1)) a b)"), "((fiat () (x) a b) 1)");
    }

    #[test]
    fn let_empty_bindings() {
        assert_eq!(desugared("(let () 42)"), "((fiat () () 42))");
    }

    #[test]
    fn and_rewrite() {
        assert_eq!(desugared("(and a b)"), "(choose (a b) (true false))");
    }

    #[test]
    fn nested_sugar_in_binding_value() {
        assert_eq!(
            desugared("(let ((x (and a b))) x)"),
            "((fiat () (x) x) (choose (a b) (true false)))"
        );
    }

    #[test]
    fn recurses_into_fiat_body() {
        assert_eq!(
            desugared("(fiat f (x) (let ((y 1)) (+ x y)))"),
            "(fiat f (x) ((fiat () (y) (+ x y)) 1))"
        );
    }

    #[test]
    fn recurses_into_choose_clauses() {
        assert_eq!(
            desugared("(choose ((and p q) r) (true s))"),
            "(choose ((choose (p q) (true false)) r) (true s))"
        );
    }

    #[test]
    fn behold_contents_untouched() {
        assert_eq!(
            desugared("(behold (let ((x 1)) x))"),
            "(behold (let ((x 1)) x))"
        );
        assert_eq!(desugared("(behold (and a b))"), "(behold (and a b))");
    }

    #[test]
    fn quote_shorthand_untouched() {
        // '(and a b) reads as (behold (and a b)) and must stay inert.
        assert_eq!(desugared("'(and a b)"), "(behold (and a b))");
    }

    #[test]
    fn let_evaluates() {
        assert_eq!(run("(let ((x 5) (y 10)) (+ x y))"), Value::Int(15));
    }

    #[test]
    fn let_sequential_scope_evaluates() {
        assert_eq!(run("(let ((x 1) (y (+ x 1))) (+ x y))"), Value::Int(3));
    }

    #[test]
    fn and_evaluates() {
        assert_eq!(run("(and true 5)"), Value::Int(5));
        assert_eq!(run("(and false 5)"), Value::Bool(false));
    }

    #[test]
    fn or_evaluates() {
        assert_eq!(run("(or false 7)"), Value::Int(7));
        assert_eq!(run("(or 3 false)"), Value::Int(3));
        assert_eq!(run("(or false false)"), Value::Bool(false));
    }

    #[test]
    fn or_is_short_circuit_structure() {
        // The desugared `or` binds the first operand once and tests it,
        // so it is a single-binding let application.
        let out = desugared("(or a b)");
        assert!(out.starts_with("((fiat () (__or_tmp"), "got: {out}");
        assert!(out.contains("(choose"), "got: {out}");
    }
}

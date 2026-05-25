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
            "behold" => return value.clone(),
            "let" if items.len() >= 2 => return desugar_let(&items),
            "and" if items.len() == 3 => return desugar_and(&items[1], &items[2]),
            "or" if items.len() == 3 => return desugar_or(&items[1], &items[2]),
            name if is_threading_op(name) && items.len() >= 3 => {
                return desugar_threading(name, &items[1..]);
            }
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

fn is_threading_op(name: &str) -> bool {
    name == "->" || name == "->>" || name.ends_with("->")
}

#[derive(Clone)]
enum ThreadMode {
    First,
    Last,
    As(String),
}

fn parse_thread_mode(name: &str) -> ThreadMode {
    match name {
        "->" => ThreadMode::First,
        "->>" => ThreadMode::Last,
        _ => {
            let binding = &name[..name.len() - 2];
            ThreadMode::As(binding.to_string())
        }
    }
}

fn desugar_threading(op_name: &str, rest: &[Value]) -> Value {
    let default_mode = parse_thread_mode(op_name);
    let init = desugar(&rest[0]);
    let steps = &rest[1..];

    let mut acc = init;
    let mut i = 0;
    while i < steps.len() {
        let (mode, step) = if let Some(s) = steps[i].as_symbol() {
            if is_threading_op(s.name()) {
                i += 1;
                if i >= steps.len() {
                    break;
                }
                (parse_thread_mode(s.name()), &steps[i])
            } else {
                (default_mode.clone(), &steps[i])
            }
        } else {
            (default_mode.clone(), &steps[i])
        };

        acc = apply_thread_step(&mode, acc, step);
        i += 1;
    }
    acc
}

fn apply_thread_step(mode: &ThreadMode, acc: Value, step: &Value) -> Value {
    let step_items = match step {
        Value::List(_) => list_to_vec(step),
        other => vec![other.clone()],
    };

    match mode {
        ThreadMode::First => {
            let mut call = vec![desugar(&step_items[0]), acc];
            call.extend(step_items[1..].iter().map(desugar));
            list_from_vec(call)
        }
        ThreadMode::Last => {
            let mut call = vec![desugar(&step_items[0])];
            call.extend(step_items[1..].iter().map(desugar));
            call.push(acc);
            list_from_vec(call)
        }
        ThreadMode::As(binding) => {
            let binding_sym = sym(binding);
            let body = if step_items.len() == 1 {
                list_from_vec(vec![desugar(&step_items[0]), binding_sym.clone()])
            } else {
                let replaced: Vec<Value> = step_items
                    .iter()
                    .map(|item| replace_binding(item, binding, &binding_sym))
                    .map(|v| desugar(&v))
                    .collect();
                list_from_vec(replaced)
            };
            desugar(&list_from_vec(vec![
                sym("let"),
                list_from_vec(vec![list_from_vec(vec![binding_sym, acc])]),
                body,
            ]))
        }
    }
}

fn replace_binding(value: &Value, binding: &str, binding_sym: &Value) -> Value {
    match value {
        Value::Symbol(s) if s.name() == binding => binding_sym.clone(),
        Value::List(_) => {
            let items = list_to_vec(value);
            list_from_vec(
                items
                    .iter()
                    .map(|v| replace_binding(v, binding, binding_sym))
                    .collect(),
            )
        }
        _ => value.clone(),
    }
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

    #[test]
    fn thread_first_basic() {
        assert_eq!(desugared("(-> x (f 1) (g 2))"), "(g (f x 1) 2)");
    }

    #[test]
    fn thread_first_bare_symbol() {
        assert_eq!(desugared("(-> x f g)"), "(g (f x))");
    }

    #[test]
    fn thread_last_basic() {
        assert_eq!(desugared("(->> x (f 1) (g 2))"), "(g 2 (f 1 x))");
    }

    #[test]
    fn thread_last_bare_symbol() {
        assert_eq!(desugared("(->> x f g)"), "(g (f x))");
    }

    #[test]
    fn thread_as_basic() {
        let out = desugared("(it-> x (+ it 2) (some-fn 1 it 3))");
        assert!(out.contains("(+ it 2)"), "got: {out}");
        assert!(out.contains("(some-fn 1 it 3)"), "got: {out}");
    }

    #[test]
    fn thread_first_per_step_override() {
        // (-> x (+ 2) ->> (- 3)) => ->> applied to (- 3) with acc = (+ x 2)
        // step 1: (+ x 2), step 2 override ->>: (- 3 (+ x 2))
        assert_eq!(desugared("(-> x (+ 2) ->> (- 3))"), "(- 3 (+ x 2))");
    }

    #[test]
    fn thread_last_per_step_override() {
        // (->> x (filter odd?) -> (nth 3))
        // step 1 ->>: (filter odd? x), step 2 override ->: (nth (filter odd? x) 3)
        assert_eq!(
            desugared("(->> x (filter odd?) -> (nth 3))"),
            "(nth (filter odd? x) 3)"
        );
    }

    #[test]
    fn thread_first_with_name_override() {
        // (-> x (+ 2) val-> (some-fn 1 val 3))
        let out = desugared("(-> x (+ 2) val-> (some-fn 1 val 3))");
        assert!(out.contains("(some-fn 1 val 3)"), "got: {out}");
    }

    #[test]
    fn thread_first_evaluates() {
        let result = run("(-> 1 (+ 2) (* 3))");
        // (-> 1 (+ 2) (* 3)) => (* (+ 1 2) 3) => (* 3 3) => 9
        assert_eq!(result, Value::Int(9));
    }

    #[test]
    fn thread_last_evaluates() {
        let result = run("(->> 10 (- 3))");
        // (->> 10 (- 3)) => (- 3 10) => -7
        assert_eq!(result, Value::Int(-7));
    }

    #[test]
    fn thread_as_evaluates() {
        let result = run("(it-> 5 (+ it it) (* it 2))");
        // step 1: it=5, (+ 5 5) = 10
        // step 2: it=10, (* 10 2) = 20
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn thread_first_nested_pipeline() {
        let result = run("(-> (-> 1 (+ 2)) (* 3))");
        // inner: (+ 1 2) = 3, outer: (* 3 3) = 9
        assert_eq!(result, Value::Int(9));
    }

    #[test]
    fn thread_first_override_evaluates() {
        let result = run("(-> 10 (+ 2) ->> (- 3))");
        // step 1 ->: (+ 10 2) = 12
        // step 2 ->>: (- 3 12) = -9
        assert_eq!(result, Value::Int(-9));
    }

    #[test]
    fn behold_inside_threading_untouched() {
        assert_eq!(desugared("(behold (-> x f g))"), "(behold (-> x f g))");
    }
}

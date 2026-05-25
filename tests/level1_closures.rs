use fiat::eval::eval_program;
use fiat::prelude;
use fiat::reader::read;
use fiat::value::Value;

fn eval_source(source: &str) -> Value {
    let forms = read(source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    eval_program(&forms, &env).expect("eval error")
}

// Church boolean definitions shared by the 1c cases.
const CHURCH: &str = r#"
    (fiat T (a b) a)
    (fiat F (a b) b)
    (fiat church-not (p) (p F T))
    (fiat church-and (p q) (p q F))
    (fiat church-or (p q) (p T q))
"#;

#[test]
fn level1a_compose() {
    let source = r#"
        (fiat compose (f g)
          (fiat () (x) (f (g x))))

        (let ((inc-then-double (compose (fiat () (x) (* x 2))
                                        (fiat () (x) (+ x 1)))))
          (inc-then-double 4))
    "#;
    assert_eq!(eval_source(source), Value::Int(10));
}

#[test]
fn level1b_currying() {
    let source = r#"
        (fiat add (a)
          (fiat () (b) (+ a b)))

        (let ((add5 (add 5)))
          (map add5 '(1 2 3 4)))
    "#;
    assert_eq!(eval_source(source).to_string(), "(6 7 8 9)");
}

#[test]
fn level1c_church_not_returns_f() {
    let source = format!("{CHURCH}\n(church-not T)");
    // (church-not T) = (T F T) = F, the function object itself.
    assert_eq!(eval_source(&source).to_string(), "<function F>");
}

#[test]
fn level1c_church_and() {
    let source = format!("{CHURCH}\n((church-and T T) 'y 'n)");
    assert_eq!(eval_source(&source).to_string(), "y");
}

#[test]
fn level1c_church_or_true() {
    let source = format!("{CHURCH}\n((church-or F T) 'y 'n)");
    assert_eq!(eval_source(&source).to_string(), "y");
}

#[test]
fn level1c_church_or_false() {
    let source = format!("{CHURCH}\n((church-or F F) 'y 'n)");
    assert_eq!(eval_source(&source).to_string(), "n");
}

#[test]
fn level1d_accumulator_factory() {
    let source = r#"
        (fiat make-counter (start)
          (fiat () ()
            (bind start (bind (make-counter (+ start 1)) ()))))

        (let ((c (make-counter 0)))
          (let ((r1 (c)))
            (let ((r2 ((first (rest r1)))))
              (first r2))))
    "#;
    assert_eq!(eval_source(source), Value::Int(1));
}

#[test]
fn level1_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level1.fiat").expect("could not read level1.fiat");
    let forms = read(&source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    let result = eval_program(&forms, &env).expect("eval error");
    // The final form is 1d, whose value is 1.
    assert_eq!(result, Value::Int(1));
}

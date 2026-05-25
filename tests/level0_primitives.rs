use fiat::env::Env;
use fiat::eval::eval_program;
use fiat::reader::read;
use fiat::value::Value;

fn eval_source(source: &str) -> Value {
    let forms = read(source).expect("read error");
    let env = Env::new();
    eval_program(&forms, &env).expect("eval error")
}

#[test]
fn level0a_roundtrip() {
    let source = r#"
        (fiat roundtrip? (lst)
          (choose
            ((atom? lst) true)
            ((is? (first (bind 'x lst)) 'x) true)
            (true false)))

        (roundtrip? '(a b c))
    "#;
    assert_eq!(eval_source(source), Value::Bool(true));
}

#[test]
fn level0b_reverse() {
    let source = r#"
        (fiat reverse (lst)
          (fiat go (remaining acc)
            (choose
              ((atom? remaining) acc)
              (true (go (rest remaining) (bind (first remaining) acc)))))
          (go lst ()))

        (reverse '(1 2 3 4 5))
    "#;
    let result = eval_source(source);
    assert_eq!(result.to_string(), "(5 4 3 2 1)");
}

#[test]
fn level0c_zip() {
    let source = r#"
        (fiat zip (xs ys)
          (choose
            ((atom? xs) ())
            ((atom? ys) ())
            (true (bind (bind (first xs) (bind (first ys) ()))
                        (zip (rest xs) (rest ys))))))

        (zip '(a b c) '(1 2 3))
    "#;
    let result = eval_source(source);
    assert_eq!(result.to_string(), "((a 1) (b 2) (c 3))");
}

#[test]
fn level0d_flatten() {
    let source = r#"
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

        (flatten '(1 (2 (3 4) 5) (6)))
    "#;
    let result = eval_source(source);
    assert_eq!(result.to_string(), "(1 2 3 4 5 6)");
}

#[test]
fn level0_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level0.fiat").expect("could not read level0.fiat");
    let forms = read(&source).expect("read error");
    let env = Env::new();

    let mut results = Vec::new();
    for form in &forms {
        let val = fiat::eval::eval(form, &env).expect("eval error");
        results.push(val);
    }

    let last = results.last().expect("no results");
    assert_eq!(last.to_string(), "(1 2 3 4 5 6)");
}

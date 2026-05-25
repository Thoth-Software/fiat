use fiat::env::Env;
use fiat::eval::eval_program;
use fiat::reader::read;
use fiat::value::Value;

const HELPERS: &str = r#"
    (fiat not (x)
      (choose
        (x false)
        (true true)))

    (fiat reverse (lst)
      (fiat go (remaining acc)
        (choose
          ((atom? remaining) acc)
          (true (go (rest remaining) (bind (first remaining) acc)))))
      (go lst ()))

    (fiat length (lst)
      (fiat go (remaining acc)
        (choose
          ((atom? remaining) acc)
          (true (go (rest remaining) (+ acc 1)))))
      (go lst 0))

    (fiat odd? (n) (not (= (% n 2) 0)))

    (fiat take (n lst)
      (choose
        ((= n 0)     ())
        ((atom? lst) ())
        (true (bind (first lst) (take (- n 1) (rest lst))))))

    (fiat drop (n lst)
      (choose
        ((= n 0)     lst)
        ((atom? lst) ())
        (true (drop (- n 1) (rest lst)))))

    (fiat nth (n lst)
      (choose
        ((atom? lst) ())
        ((= n 0)     (first lst))
        (true        (nth (- n 1) (rest lst)))))

    (fiat zip-with (f xs ys)
      (choose
        ((atom? xs) ())
        ((atom? ys) ())
        (true (bind (f (first xs) (first ys))
                    (zip-with f (rest xs) (rest ys))))))

    (fiat any? (pred lst)
      (choose
        ((atom? lst) false)
        ((pred (first lst)) true)
        (true (any? pred (rest lst)))))

    (fiat all? (pred lst)
      (choose
        ((atom? lst) true)
        ((not (pred (first lst))) false)
        (true (all? pred (rest lst)))))

    (fiat partition (pred lst)
      (fiat go (remaining yes no)
        (choose
          ((atom? remaining) (bind (reverse yes) (bind (reverse no) ())))
          ((pred (first remaining))
            (go (rest remaining) (bind (first remaining) yes) no))
          (true
            (go (rest remaining) yes (bind (first remaining) no)))))
      (go lst () ()))

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
"#;

fn eval_source(source: &str) -> Value {
    let forms = read(source).expect("read error");
    let env = Env::new();
    eval_program(&forms, &env).expect("eval error")
}

#[test]
fn level6_take() {
    let source = format!("{HELPERS} (take 3 '(a b c d e))");
    assert_eq!(eval_source(&source).to_string(), "(a b c)");
}

#[test]
fn level6_take_more_than_length() {
    let source = format!("{HELPERS} (take 10 '(1 2 3))");
    assert_eq!(eval_source(&source).to_string(), "(1 2 3)");
}

#[test]
fn level6_take_zero() {
    let source = format!("{HELPERS} (take 0 '(1 2 3))");
    assert_eq!(eval_source(&source), Value::Nil);
}

#[test]
fn level6_drop() {
    let source = format!("{HELPERS} (drop 2 '(a b c d e))");
    assert_eq!(eval_source(&source).to_string(), "(c d e)");
}

#[test]
fn level6_drop_all() {
    let source = format!("{HELPERS} (drop 10 '(1 2))");
    assert_eq!(eval_source(&source), Value::Nil);
}

#[test]
fn level6_nth() {
    let source = format!("{HELPERS} (nth 0 '(a b c))");
    assert_eq!(eval_source(&source).to_string(), "a");
}

#[test]
fn level6_nth_middle() {
    let source = format!("{HELPERS} (nth 2 '(10 20 30 40))");
    assert_eq!(eval_source(&source), Value::Int(30));
}

#[test]
fn level6_nth_out_of_bounds() {
    let source = format!("{HELPERS} (nth 5 '(1 2))");
    assert_eq!(eval_source(&source), Value::Nil);
}

#[test]
fn level6_zip_with() {
    let source = format!("{HELPERS} (zip-with + '(1 2 3) '(10 20 30))");
    assert_eq!(eval_source(&source).to_string(), "(11 22 33)");
}

#[test]
fn level6_zip_with_unequal_lengths() {
    let source = format!("{HELPERS} (zip-with + '(1 2) '(10 20 30))");
    assert_eq!(eval_source(&source).to_string(), "(11 22)");
}

#[test]
fn level6_any_true() {
    let source = format!("{HELPERS} (any? odd? '(2 4 5 6))");
    assert_eq!(eval_source(&source), Value::Bool(true));
}

#[test]
fn level6_any_false() {
    let source = format!("{HELPERS} (any? odd? '(2 4 6))");
    assert_eq!(eval_source(&source), Value::Bool(false));
}

#[test]
fn level6_any_empty() {
    let source = format!("{HELPERS} (any? odd? '())");
    assert_eq!(eval_source(&source), Value::Bool(false));
}

#[test]
fn level6_all_true() {
    let source = format!("{HELPERS} (all? odd? '(1 3 5))");
    assert_eq!(eval_source(&source), Value::Bool(true));
}

#[test]
fn level6_all_false() {
    let source = format!("{HELPERS} (all? odd? '(1 2 3))");
    assert_eq!(eval_source(&source), Value::Bool(false));
}

#[test]
fn level6_all_empty() {
    let source = format!("{HELPERS} (all? odd? '())");
    assert_eq!(eval_source(&source), Value::Bool(true));
}

#[test]
fn level6_partition() {
    let source = format!("{HELPERS} (partition odd? '(1 2 3 4 5 6))");
    assert_eq!(eval_source(&source).to_string(), "((1 3 5) (2 4 6))");
}

#[test]
fn level6_partition_all_match() {
    let source = format!("{HELPERS} (partition odd? '(1 3 5))");
    assert_eq!(eval_source(&source).to_string(), "((1 3 5) ())");
}

#[test]
fn level6_partition_none_match() {
    let source = format!("{HELPERS} (partition odd? '(2 4 6))");
    assert_eq!(eval_source(&source).to_string(), "(() (2 4 6))");
}

#[test]
fn level6_sort() {
    let source = format!("{HELPERS} (sort < '(5 3 8 1 9 2 7))");
    assert_eq!(eval_source(&source).to_string(), "(1 2 3 5 7 8 9)");
}

#[test]
fn level6_sort_descending() {
    let source = format!("{HELPERS} (sort > '(5 3 8 1 9 2 7))");
    assert_eq!(eval_source(&source).to_string(), "(9 8 7 5 3 2 1)");
}

#[test]
fn level6_sort_already_sorted() {
    let source = format!("{HELPERS} (sort < '(1 2 3))");
    assert_eq!(eval_source(&source).to_string(), "(1 2 3)");
}

#[test]
fn level6_sort_single() {
    let source = format!("{HELPERS} (sort < '(42))");
    assert_eq!(eval_source(&source).to_string(), "(42)");
}

#[test]
fn level6_sort_empty() {
    let source = format!("{HELPERS} (sort < '())");
    assert_eq!(eval_source(&source), Value::Nil);
}

#[test]
fn level6_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level6.fiat").expect("could not read level6.fiat");
    let forms = read(&source).expect("read error");
    let env = Env::new();
    let result = eval_program(&forms, &env).expect("eval error");
    assert_eq!(result.to_string(), "(1 2 3 5 7 8 9)");
}

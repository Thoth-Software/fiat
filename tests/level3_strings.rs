use std::rc::Rc;

use fiat::eval::eval_program;
use fiat::prelude;
use fiat::reader::read;
use fiat::value::Value;

fn eval_source(source: &str) -> Value {
    let forms = read(source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    eval_program(&forms, &env).expect("eval error")
}

// --- 3a. Slugify ---

#[test]
fn level3a_slugify() {
    let source = r#"
        (fiat Lux)

        (fiat slugify (title)
          (-> title
              String/downcase
              String/trim
              (String/replace " " "-")
              (String/replace "'" "")))

        (slugify " Hello World's End ")
    "#;
    let result = eval_source(source);
    assert_eq!(result, Value::String(Rc::from("hello-worlds-end")));
}

// --- 3b. Palindrome check ---

#[test]
fn level3b_palindrome_true() {
    let source = r#"
        (fiat Lux)

        (fiat lists-equal? (xs ys)
          (choose
            ((atom? xs) (atom? ys))
            ((atom? ys) false)
            ((not (is? (first xs) (first ys))) false)
            (true (lists-equal? (rest xs) (rest ys)))))

        (fiat palindrome? (s)
          (let ((cps (-> s String/downcase String/trim as-codepoints)))
            (lists-equal? cps (reverse cps))))

        (palindrome? "racecar")
    "#;
    let result = eval_source(source);
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn level3b_palindrome_false() {
    let source = r#"
        (fiat Lux)

        (fiat lists-equal? (xs ys)
          (choose
            ((atom? xs) (atom? ys))
            ((atom? ys) false)
            ((not (is? (first xs) (first ys))) false)
            (true (lists-equal? (rest xs) (rest ys)))))

        (fiat palindrome? (s)
          (let ((cps (-> s String/downcase String/trim as-codepoints)))
            (lists-equal? cps (reverse cps))))

        (palindrome? "hello")
    "#;
    let result = eval_source(source);
    assert_eq!(result, Value::Bool(false));
}

// --- 3c. Long words ---

#[test]
fn level3c_long_words() {
    let source = r#"
        (fiat Lux)

        (fiat long-words (text min-len)
          (->> (String/split text " ")
               (filter (fiat () (w) (> (String/length w) min-len)))))

        (long-words "the quick brown fox jumped" 4)
    "#;
    let result = eval_source(source);
    assert_eq!(result.to_string(), r#"("quick" "brown" "jumped")"#);
}

// --- 3d. Caesar cipher ---

#[test]
fn level3d_caesar_encrypt() {
    let source = r#"
        (fiat Lux)

        (fiat caesar-encrypt (text shift)
          (let ((encrypt-char
                  (fiat () (cp)
                    (choose
                      ((and (>= cp 97) (<= cp 122))
                        (+ 97 (% (+ (- cp 97) shift) 26)))
                      ((and (>= cp 65) (<= cp 90))
                        (+ 65 (% (+ (- cp 65) shift) 26)))
                      (true cp)))))
            (-> text
                as-codepoints
                ->> (map encrypt-char)
                from-codepoints)))

        (caesar-encrypt "Hello" 3)
    "#;
    let result = eval_source(source);
    assert_eq!(result, Value::String(Rc::from("Khoor")));
}

#[test]
fn level3d_caesar_roundtrip() {
    let source = r#"
        (fiat Lux)

        (fiat caesar-encrypt (text shift)
          (let ((encrypt-char
                  (fiat () (cp)
                    (choose
                      ((and (>= cp 97) (<= cp 122))
                        (+ 97 (% (+ (- cp 97) shift) 26)))
                      ((and (>= cp 65) (<= cp 90))
                        (+ 65 (% (+ (- cp 65) shift) 26)))
                      (true cp)))))
            (-> text
                as-codepoints
                ->> (map encrypt-char)
                from-codepoints)))

        (caesar-encrypt (caesar-encrypt "Hello" 3) 23)
    "#;
    let result = eval_source(source);
    assert_eq!(result, Value::String(Rc::from("Hello")));
}

// --- Full file ---

#[test]
fn level3_full_file() {
    let source =
        std::fs::read_to_string("benchmarks/level3.fiat").expect("could not read level3.fiat");
    let forms = read(&source).expect("read error");
    let env = prelude::environment().expect("prelude load");
    let result = eval_program(&forms, &env).expect("eval error");
    assert_eq!(result, Value::String(Rc::from("Hello")));
}

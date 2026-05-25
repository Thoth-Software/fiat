use std::rc::Rc;

use crate::env::Env;
use crate::error::Error;
use crate::eval::eval_program;
use crate::reader::read;

/// The self-hosted prelude source, embedded so there is no runtime file
/// dependency.
const PRELUDE_SOURCE: &str = include_str!("prelude.fiat");

/// Evaluate the prelude into an existing environment, defining the standard
/// helper functions (`map`, `filter`, `fold`, `not`, `>=`, `<=`, `max`,
/// `min`, `length`, `reverse`, `append`).
pub fn load(env: &Rc<Env>) -> Result<(), Error> {
    let forms = read(PRELUDE_SOURCE)?;
    eval_program(&forms, env)?;
    Ok(())
}

/// Create a fresh top-level environment with the prelude loaded.
///
/// Use [`Env::new`] directly for a bare environment (Levels 0 and 6 run
/// without the prelude).
pub fn environment() -> Result<Rc<Env>, Error> {
    let env = Env::new();
    load(&env)?;
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn run(source: &str) -> Value {
        let env = environment().expect("prelude should load");
        let forms = read(source).expect("read error");
        eval_program(&forms, &env).expect("eval error")
    }

    fn run_str(source: &str) -> String {
        run(source).to_string()
    }

    #[test]
    fn not_negates() {
        assert_eq!(run("(not true)"), Value::Bool(false));
        assert_eq!(run("(not false)"), Value::Bool(true));
    }

    #[test]
    fn gte_and_lte() {
        assert_eq!(run("(>= 3 3)"), Value::Bool(true));
        assert_eq!(run("(>= 2 3)"), Value::Bool(false));
        assert_eq!(run("(>= 4 3)"), Value::Bool(true));
        assert_eq!(run("(<= 3 3)"), Value::Bool(true));
        assert_eq!(run("(<= 4 3)"), Value::Bool(false));
    }

    #[test]
    fn max_and_min() {
        assert_eq!(run("(max 2 5)"), Value::Int(5));
        assert_eq!(run("(max 5 2)"), Value::Int(5));
        assert_eq!(run("(min 2 5)"), Value::Int(2));
    }

    #[test]
    fn fold_sums() {
        // `+` is passed as a first-class value here.
        assert_eq!(run("(fold + 0 '(1 2 3))"), Value::Int(6));
    }

    #[test]
    fn fold_with_prelude_function() {
        assert_eq!(run("(fold max 0 '(1 5 3 2))"), Value::Int(5));
    }

    #[test]
    fn map_doubles() {
        assert_eq!(run_str("(map (fiat () (x) (* x 2)) '(1 2 3))"), "(2 4 6)");
    }

    #[test]
    fn filter_keeps_matching() {
        assert_eq!(
            run_str("(filter (fiat () (x) (> x 2)) '(1 2 3 4))"),
            "(3 4)"
        );
    }

    #[test]
    fn length_counts() {
        assert_eq!(run("(length '(a b c))"), Value::Int(3));
        assert_eq!(run("(length ())"), Value::Int(0));
    }

    #[test]
    fn reverse_reverses() {
        assert_eq!(run_str("(reverse '(1 2 3))"), "(3 2 1)");
        assert_eq!(run_str("(reverse ())"), "()");
    }

    #[test]
    fn append_concatenates() {
        assert_eq!(run_str("(append '(1 2) '(3 4))"), "(1 2 3 4)");
    }

    #[test]
    fn bare_env_has_no_prelude() {
        let env = Env::new();
        let forms = read("(map (fiat () (x) x) '(1 2 3))").expect("read error");
        // `map` is not defined in a bare environment.
        assert!(eval_program(&forms, &env).is_err());
    }
}

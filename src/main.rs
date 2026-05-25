use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;

use fiat::env::Env;
use fiat::error::Error;
use fiat::eval::eval_program;
use fiat::prelude;
use fiat::reader::{read, read_one};
use fiat::value::Value;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let no_prelude = args.iter().any(|a| a.as_str() == "--no-prelude");
    let path = args.iter().find(|a| !a.starts_with("--"));

    path.map_or_else(|| run_repl(no_prelude), |path| run_file(path, no_prelude))
}

/// Build the top-level environment: the prelude is loaded by default, but
/// `--no-prelude` gives a bare environment (Levels 0 and 6 run without it).
/// Standalone mode registers Firmamentum so scripts can use I/O capabilities.
fn make_env(no_prelude: bool) -> Result<Rc<Env>, Error> {
    let env = if no_prelude {
        Env::new()
    } else {
        prelude::environment()?
    };
    env.register_capability("Firmamentum".to_string());
    Ok(env)
}

/// Read and evaluate a whole program, returning the value of the final form.
fn run_source(source: &str, env: &Rc<Env>) -> Result<Value, Error> {
    let forms = read(source)?;
    eval_program(&forms, env)
}

fn run_file(path: &str, no_prelude: bool) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            return ExitCode::FAILURE;
        }
    };
    let env = match make_env(no_prelude) {
        Ok(env) => env,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    match run_source(&source, &env) {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_repl(no_prelude: bool) -> ExitCode {
    let env = match make_env(no_prelude) {
        Ok(env) => env,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        prompt();
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => break, // EOF (Ctrl-D)
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match read_one(trimmed) {
                    // eval_program desugars before evaluating, so `let`,
                    // `and`, and `or` work at the REPL.
                    Ok(form) => match eval_program(std::slice::from_ref(&form), &env) {
                        Ok(value) => println!("{value}"),
                        Err(err) => eprintln!("error: {err}"),
                    },
                    Err(err) => eprintln!("error: {err}"),
                }
            }
            Err(err) => {
                eprintln!("error: {err}");
                break;
            }
        }
    }
    ExitCode::SUCCESS
}

fn prompt() {
    print!("fiat> ");
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_arithmetic() {
        let env = make_env(false).expect("env");
        let value = run_source("(+ 1 2)", &env).expect("eval");
        assert_eq!(value, Value::Int(3));
    }

    #[test]
    fn prelude_available_by_default() {
        let env = make_env(false).expect("env");
        let value = run_source("(map (fiat () (x) (* x 2)) '(1 2 3))", &env).expect("eval");
        assert_eq!(value.to_string(), "(2 4 6)");
    }

    #[test]
    fn no_prelude_is_bare() {
        let env = make_env(true).expect("env");
        // `map` is a prelude function, absent from a bare environment.
        assert!(run_source("(map (fiat () (x) x) '(1 2))", &env).is_err());
    }

    #[test]
    fn runs_level0_benchmark_file() {
        let source = std::fs::read_to_string("benchmarks/level0.fiat").expect("read file");
        // Level 0 defines its own helpers, so it runs under either environment.
        let with_prelude = make_env(false).expect("env");
        assert_eq!(
            run_source(&source, &with_prelude)
                .expect("eval")
                .to_string(),
            "(1 2 3 4 5 6)"
        );
        let bare = make_env(true).expect("env");
        assert_eq!(
            run_source(&source, &bare).expect("eval").to_string(),
            "(1 2 3 4 5 6)"
        );
    }
}

use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use fiat::env::Env;
use fiat::error::Error;
use fiat::eval::eval_program;
use fiat::prelude;
use fiat::reader::read;
use fiat::value::Value;

use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
Fiat — a homoiconic Lisp interpreter.

Usage:
  fiat                 Start the REPL (prelude loaded)
  fiat lux             Start the REPL with the Lux standard library imported
  fiat <file.fiat>     Run a Fiat source file
  fiat --no-prelude …  Use a bare environment (no map/filter/fold/sort)

Options:
  --no-prelude   Omit the self-hosted prelude (Levels 0 and 6 run bare)
  -h, --help     Show this help
  -V, --version  Show the version

In the REPL, type a form and press Enter; multi-line forms continue
until balanced. Press Ctrl-D to exit. History is saved to ~/.fiat_history.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("fiat {VERSION}");
        return ExitCode::SUCCESS;
    }

    let no_prelude = args.iter().any(|a| a == "--no-prelude");
    let positional = args.iter().find(|a| !a.starts_with('-'));

    match positional.map(String::as_str) {
        Some("lux") => run_repl(no_prelude, true),
        Some(path) => run_file(path, no_prelude),
        None => run_repl(no_prelude, false),
    }
}

/// Build the top-level environment. The prelude is loaded by default, but
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

/// Build a REPL environment, optionally pre-importing the Lux standard library
/// so interactive sessions can call `String/*`, `Map/*`, `Math/*`, etc. without
/// typing `(fiat Lux)` first.
fn make_repl_env(no_prelude: bool, import_lux: bool) -> Result<Rc<Env>, Error> {
    let env = make_env(no_prelude)?;
    if import_lux {
        eval_program(&read("(fiat Lux)")?, &env)?;
    }
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

/// True when a read error indicates the input is incomplete rather than
/// malformed, so the REPL should keep reading more lines.
fn is_incomplete(err: &Error) -> bool {
    let msg = &err.message;
    msg.starts_with("unterminated") || msg == "unexpected end of input"
}

fn history_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".fiat_history"))
}

fn run_repl(no_prelude: bool, import_lux: bool) -> ExitCode {
    let env = match make_repl_env(no_prelude, import_lux) {
        Ok(env) => env,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut editor = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(err) => {
            eprintln!("error: could not start REPL: {err}");
            return ExitCode::FAILURE;
        }
    };
    let history = history_path();
    if let Some(path) = &history {
        let _ = editor.load_history(path);
    }

    println!("Fiat {VERSION} REPL — Ctrl-D to exit");
    if import_lux {
        println!("Lux standard library imported.");
    }

    let exit = repl_loop(&mut editor, &env);

    if let Some(path) = &history {
        let _ = editor.save_history(path);
    }
    exit
}

fn repl_loop(editor: &mut DefaultEditor, env: &Rc<Env>) -> ExitCode {
    loop {
        let mut buffer = String::new();
        let mut prompt: &str = "fiat> ";

        let complete = loop {
            match editor.readline(prompt) {
                Ok(line) => {
                    buffer.push_str(&line);
                    buffer.push('\n');
                    match read(&buffer) {
                        Ok(forms) => break Some(forms),
                        Err(err) if is_incomplete(&err) => {
                            prompt = "  ... ";
                        }
                        Err(err) => {
                            eprintln!("error: {err}");
                            break None;
                        }
                    }
                }
                // Ctrl-C abandons the current input and starts a fresh prompt.
                Err(ReadlineError::Interrupted) => break None,
                // Ctrl-D exits the REPL.
                Err(ReadlineError::Eof) => return ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("error: {err}");
                    return ExitCode::FAILURE;
                }
            }
        };

        let trimmed = buffer.trim();
        if !trimmed.is_empty() {
            let _ = editor.add_history_entry(trimmed);
        }

        if let Some(forms) = complete {
            for form in &forms {
                match eval_program(std::slice::from_ref(form), env) {
                    Ok(value) => println!("{value}"),
                    Err(err) => eprintln!("error: {err}"),
                }
            }
        }
    }
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
    fn repl_lux_import_makes_stdlib_available() {
        // `fiat lux` pre-imports Lux, so String/* resolve without `(fiat Lux)`.
        let env = make_repl_env(false, true).expect("env");
        let value = run_source("(String/upcase \"hi\")", &env).expect("eval");
        assert_eq!(value, Value::String(std::rc::Rc::from("HI")));
    }

    #[test]
    fn repl_without_lux_import_lacks_stdlib() {
        // A plain REPL does not import Lux automatically.
        let env = make_repl_env(false, false).expect("env");
        assert!(run_source("(String/upcase \"hi\")", &env).is_err());
    }

    #[test]
    fn incomplete_input_is_detected() {
        let err = read("(+ 1").expect_err("should be incomplete");
        assert!(is_incomplete(&err));
    }

    #[test]
    fn malformed_input_is_not_incomplete() {
        let err = read(")").expect_err("should be malformed");
        assert!(!is_incomplete(&err));
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

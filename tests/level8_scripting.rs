use std::sync::Mutex;

use fiat::eval::eval_program;
use fiat::prelude;
use fiat::reader::read;
use fiat::value::{InternedSymbol, Value};

// Serializes the one test that changes the process-wide current directory.
static CWD_LOCK: Mutex<()> = Mutex::new(());

const EXPECTED_REPORT: &str = "Marketing: 1 employees, avg $70000, range $70000-$70000\n\
     Sales: 3 employees, avg $85000, range $80000-$90000\n\
     Engineering: 2 employees, avg $110000, range $100000-$120000";

fn kw(name: &str) -> Value {
    Value::Keyword(InternedSymbol::new(name))
}

fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut dir = std::env::temp_dir();
    dir.push(format!("fiat_level8_{tag}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// Resolve repo files to absolute paths so reads do not depend on the current
// directory (the full-file test temporarily changes it).
fn repo_path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn fixture_csv() -> String {
    std::fs::read_to_string(repo_path("benchmarks/employees.csv")).expect("read fixture")
}

fn benchmark_source() -> String {
    std::fs::read_to_string(repo_path("benchmarks/level8.fiat")).expect("read benchmark")
}

/// The benchmark program with its hardcoded paths swapped for the given
/// absolute temp paths, so the test is hermetic.
fn pipeline_with_paths(input: &str, output: &str) -> String {
    benchmark_source()
        .replace("\"employees.csv\"", &format!("\"{input}\""))
        .replace("\"report.txt\"", &format!("\"{output}\""))
}

fn run(source: &str) -> Value {
    let env = prelude::scripting_environment().expect("scripting env");
    let forms = read(source).expect("read error");
    eval_program(&forms, &env).expect("eval error")
}

#[test]
fn level8_success_writes_report() {
    let dir = unique_temp_dir("success");
    let input = dir.join("employees.csv");
    let output = dir.join("report.txt");
    std::fs::write(&input, fixture_csv()).expect("write input");

    let source = pipeline_with_paths(
        input.to_str().expect("utf8"),
        output.to_str().expect("utf8"),
    );
    let result = run(&source);

    let Value::Map(m) = &result else {
        panic!("expected result map, got {result}");
    };
    assert_eq!(m.get(&kw("ok")), Some(&Value::Bool(true)));

    let written = std::fs::read_to_string(&output).expect("report written");
    assert_eq!(written, EXPECTED_REPORT);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn level8_missing_input_short_circuits() {
    let dir = unique_temp_dir("missing");
    let input = dir.join("does-not-exist.csv");
    let output = dir.join("report.txt");

    let source = pipeline_with_paths(
        input.to_str().expect("utf8"),
        output.to_str().expect("utf8"),
    );
    let result = run(&source);

    let Value::Map(m) = &result else {
        panic!("expected result map, got {result}");
    };
    assert!(m.contains_key(&kw("err")), "expected :err, got {result}");
    assert!(!m.contains_key(&kw("ok")));
    assert!(
        !output.exists(),
        "no report should be written on read failure"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn level8_full_file_in_temp_cwd() {
    let _guard = CWD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let dir = unique_temp_dir("fullfile");
    std::fs::write(dir.join("employees.csv"), fixture_csv()).expect("write input");
    let benchmark = benchmark_source();

    let original = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&dir).expect("set cwd");

    let env = prelude::scripting_environment().expect("scripting env");
    let forms = read(&benchmark).expect("read error");
    let result = eval_program(&forms, &env);

    // Restore cwd before any assertion can unwind.
    std::env::set_current_dir(&original).expect("restore cwd");

    let result = result.expect("eval error");
    let Value::Map(m) = &result else {
        panic!("expected result map, got {result}");
    };
    assert_eq!(m.get(&kw("ok")), Some(&Value::Bool(true)));

    let written = std::fs::read_to_string(dir.join("report.txt")).expect("report written");
    assert_eq!(written, EXPECTED_REPORT);

    std::fs::remove_dir_all(&dir).ok();
}

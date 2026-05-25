use std::rc::Rc;

use im_rc::HashMap as PersistentMap;

use crate::env::Env;
use crate::error::Error;
use crate::value::{Builtin, BuiltinFn, InternedSymbol, Value};

struct Entry {
    name: &'static str,
    func: BuiltinFn,
}

fn ok(value: Value) -> Value {
    let mut m = PersistentMap::new();
    m.insert(Value::Keyword(InternedSymbol::new("ok")), value);
    Value::Map(Rc::new(m))
}

fn err(reason: &str) -> Value {
    let mut m = PersistentMap::new();
    m.insert(
        Value::Keyword(InternedSymbol::new("err")),
        Value::String(Rc::from(reason)),
    );
    Value::Map(Rc::new(m))
}

/// `(Fs/read path)` → `{:ok contents}` or `{:err reason}`.
fn fs_read(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Fs/read", 1, args.len()));
    }
    let Value::String(path) = &args[0] else {
        return Err(Error::type_error("string", args[0].type_name()));
    };
    match std::fs::read_to_string(path.as_ref()) {
        Ok(contents) => Ok(ok(Value::String(Rc::from(contents.as_str())))),
        Err(e) => Ok(err(&e.to_string())),
    }
}

/// `(Fs/write contents path)` → `{:ok true}` or `{:err reason}`.
/// Note the argument order: contents first, path second.
fn fs_write(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("Fs/write", 2, args.len()));
    }
    let Value::String(contents) = &args[0] else {
        return Err(Error::type_error("string", args[0].type_name()));
    };
    let Value::String(path) = &args[1] else {
        return Err(Error::type_error("string", args[1].type_name()));
    };
    match std::fs::write(path.as_ref(), contents.as_bytes()) {
        Ok(()) => Ok(ok(Value::Bool(true))),
        Err(e) => Ok(err(&e.to_string())),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn not_implemented(args: &[Value]) -> Result<Value, Error> {
    let _ = args;
    Ok(err("not implemented"))
}

fn firmamentum_entries() -> &'static [Entry] {
    &[
        Entry {
            name: "Fs/read",
            func: fs_read,
        },
        Entry {
            name: "Fs/write",
            func: fs_write,
        },
        Entry {
            name: "Process/exit",
            func: not_implemented,
        },
        Entry {
            name: "Net/connect",
            func: not_implemented,
        },
        Entry {
            name: "Http/get",
            func: not_implemented,
        },
    ]
}

#[allow(clippy::unnecessary_wraps)]
pub fn import(env: &Rc<Env>) -> Result<Value, Error> {
    for entry in firmamentum_entries() {
        env.set(
            InternedSymbol::new(entry.name),
            Value::Builtin(Builtin {
                name: entry.name,
                func: entry.func,
            }),
        );
    }
    Ok(Value::Nil)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_path(tag: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        path.push(format!("fiat_fs_test_{tag}_{nanos}.txt"));
        path
    }

    fn kw(name: &str) -> Value {
        Value::Keyword(InternedSymbol::new(name))
    }

    #[test]
    fn write_then_read_roundtrip() {
        let path = unique_temp_path("roundtrip");
        let path_str = Value::String(Rc::from(path.to_str().expect("utf8 path")));
        let contents = Value::String(Rc::from("hello world"));

        let write_result = fs_write(&[contents, path_str.clone()]).expect("write");
        let Value::Map(m) = &write_result else {
            panic!("expected map from write");
        };
        assert_eq!(m.get(&kw("ok")), Some(&Value::Bool(true)));

        let read_result = fs_read(&[path_str]).expect("read");
        let Value::Map(m) = &read_result else {
            panic!("expected map from read");
        };
        assert_eq!(
            m.get(&kw("ok")),
            Some(&Value::String(Rc::from("hello world")))
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_missing_file_is_err() {
        let path = unique_temp_path("missing");
        let path_str = Value::String(Rc::from(path.to_str().expect("utf8 path")));
        let result = fs_read(&[path_str]).expect("read returns result map");
        let Value::Map(m) = &result else {
            panic!("expected map from read");
        };
        assert!(m.contains_key(&kw("err")), "expected :err for missing file");
        assert!(!m.contains_key(&kw("ok")));
    }

    #[test]
    fn read_type_error_on_non_string() {
        assert!(fs_read(&[Value::Int(1)]).is_err());
    }

    #[test]
    fn write_type_error_on_non_string_path() {
        let contents = Value::String(Rc::from("x"));
        assert!(fs_write(&[contents, Value::Int(1)]).is_err());
    }

    #[test]
    fn stubs_return_err_maps() {
        let result = not_implemented(&[]).expect("stub returns result map");
        let Value::Map(m) = &result else {
            panic!("expected map from stub");
        };
        assert!(m.contains_key(&kw("err")));
    }
}

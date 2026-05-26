mod firmamentum;
mod float;
mod int;
mod map;
mod math;
mod string;
mod vector;

use std::rc::Rc;

use crate::env::Env;
use crate::error::Error;
use crate::eval::eval_program;
use crate::reader::read;
use crate::value::{Builtin, BuiltinFn, InternedSymbol, Value};

/// Self-hosted Lux modules, written in Fiat and embedded so there is no
/// runtime file dependency. These wrap kernel primitives and prelude
/// functions under their documented namespaces.
const SET_SOURCE: &str = include_str!("../../lib/Set.fiat");

struct Entry {
    name: &'static str,
    func: BuiltinFn,
}

fn lux_entries() -> &'static [Entry] {
    &[
        Entry {
            name: "Int/to-string",
            func: int::to_string,
        },
        Entry {
            name: "Int/parse",
            func: int::parse,
        },
        Entry {
            name: "Float/to-string",
            func: float::to_string,
        },
        Entry {
            name: "Math/sqrt",
            func: math::sqrt,
        },
        Entry {
            name: "Map/get",
            func: map::get,
        },
        Entry {
            name: "Map/put",
            func: map::put,
        },
        Entry {
            name: "Map/merge",
            func: map::merge,
        },
        Entry {
            name: "Map/entries",
            func: map::entries,
        },
        Entry {
            name: "Map/map-values",
            func: map::map_values,
        },
        Entry {
            name: "Vector/append",
            func: vector::append,
        },
        Entry {
            name: "Vector/nth",
            func: vector::nth,
        },
        Entry {
            name: "Vector/to-list",
            func: vector::to_list,
        },
        Entry {
            name: "String/downcase",
            func: string::downcase,
        },
        Entry {
            name: "String/upcase",
            func: string::upcase,
        },
        Entry {
            name: "String/trim",
            func: string::trim,
        },
        Entry {
            name: "String/replace",
            func: string::replace,
        },
        Entry {
            name: "String/split",
            func: string::split,
        },
        Entry {
            name: "String/join",
            func: string::join,
        },
        Entry {
            name: "String/concat",
            func: string::concat,
        },
        Entry {
            name: "String/length",
            func: string::length,
        },
        Entry {
            name: "String/starts-with?",
            func: string::starts_with,
        },
    ]
}

pub fn import_lux(env: &Rc<Env>) -> Result<Value, Error> {
    for entry in lux_entries() {
        env.set(
            InternedSymbol::new(entry.name),
            Value::Builtin(Builtin {
                name: entry.name,
                func: entry.func,
            }),
        );
    }
    let forms = read(SET_SOURCE)?;
    eval_program(&forms, env)?;
    Ok(Value::Nil)
}

pub fn import_module(name: &str, env: &Rc<Env>) -> Result<Value, Error> {
    match name {
        "Lux" => import_lux(env),
        "Firmamentum" => {
            if !env.has_capability("Firmamentum") {
                return Err(Error::runtime(
                    "module unavailable: Firmamentum (not registered by host)",
                ));
            }
            firmamentum::import(env)
        }
        _ => Err(Error::runtime(format!("unknown module: {name}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_with_lux(source: &str) -> Value {
        let env = crate::prelude::environment().expect("prelude should load");
        let lux = read("(fiat Lux)").expect("read (fiat Lux)");
        eval_program(&lux, &env).expect("import Lux");
        let forms = read(source).expect("read source");
        eval_program(&forms, &env).expect("eval source")
    }

    #[test]
    fn set_module_namespaces_primitives() {
        assert_eq!(eval_with_lux("(Set/set? #{1 2})"), Value::Bool(true));
        assert_eq!(eval_with_lux("(Set/has? 2 #{1 2})"), Value::Bool(true));
        assert_eq!(eval_with_lux("(Set/has? 9 #{1 2})"), Value::Bool(false));
        // union / intersect / without return sets; probe the result by membership.
        assert_eq!(
            eval_with_lux("(Set/has? 3 (Set/union #{1 2} #{3}))"),
            Value::Bool(true)
        );
        assert_eq!(
            eval_with_lux("(Set/has? 2 (Set/intersect #{1 2} #{2 3}))"),
            Value::Bool(true)
        );
        assert_eq!(
            eval_with_lux("(Set/has? 1 (Set/intersect #{1 2} #{2 3}))"),
            Value::Bool(false)
        );
        assert_eq!(
            eval_with_lux("(Set/has? 1 (Set/without #{1 2} #{1}))"),
            Value::Bool(false)
        );
        assert_eq!(
            eval_with_lux("(Set/has? 2 (Set/without #{1 2} #{1}))"),
            Value::Bool(true)
        );
    }
}

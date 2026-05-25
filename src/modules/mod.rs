mod float;
mod int;
mod map;
mod math;
mod string;
mod vector;

use std::rc::Rc;

use crate::env::Env;
use crate::error::Error;
use crate::value::{Builtin, BuiltinFn, InternedSymbol, Value};

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
    Ok(Value::Nil)
}

pub fn import_module(name: &str, env: &Rc<Env>) -> Result<Value, Error> {
    match name {
        "Lux" => import_lux(env),
        _ => Err(Error::runtime(format!("unknown module: {name}"))),
    }
}

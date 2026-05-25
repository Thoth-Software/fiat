mod map;
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
            func: int_to_string,
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

// --- Int namespace ---

fn int_to_string(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Int/to-string", 1, args.len()));
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::String(Rc::from(n.to_string().as_str()))),
        _ => Err(Error::type_error("int", args[0].type_name())),
    }
}

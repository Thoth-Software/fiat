use std::rc::Rc;

use crate::env::Env;
use crate::error::Error;
use crate::value::{Builtin, InternedSymbol, Value};

use crate::value::BuiltinFn;

struct Entry {
    name: &'static str,
    func: BuiltinFn,
}

fn not_implemented(args: &[Value]) -> Result<Value, Error> {
    let _ = args;
    Err(Error::runtime("not implemented"))
}

fn firmamentum_entries() -> &'static [Entry] {
    &[
        Entry {
            name: "Fs/read",
            func: not_implemented,
        },
        Entry {
            name: "Fs/write",
            func: not_implemented,
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

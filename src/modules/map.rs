use std::rc::Rc;

use im_rc::HashMap as PersistentMap;

use crate::error::Error;
use crate::eval::apply;
use crate::value::{Value, list_from_vec};

pub fn get(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 3 {
        return Err(Error::arity("Map/get", 3, args.len()));
    }
    match &args[0] {
        Value::Map(m) => Ok(m.get(&args[1]).cloned().unwrap_or_else(|| args[2].clone())),
        _ => Err(Error::type_error("map", args[0].type_name())),
    }
}

pub fn put(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 3 {
        return Err(Error::arity("Map/put", 3, args.len()));
    }
    match &args[0] {
        Value::Map(m) => {
            let updated = (**m).clone().update(args[1].clone(), args[2].clone());
            Ok(Value::Map(Rc::new(updated)))
        }
        _ => Err(Error::type_error("map", args[0].type_name())),
    }
}

pub fn merge(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("Map/merge", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::Map(a), Value::Map(b)) => {
            let merged = (**b).clone().union((**a).clone());
            Ok(Value::Map(Rc::new(merged)))
        }
        (Value::Map(_), _) => Err(Error::type_error("map", args[1].type_name())),
        _ => Err(Error::type_error("map", args[0].type_name())),
    }
}

pub fn entries(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Map/entries", 1, args.len()));
    }
    match &args[0] {
        Value::Map(m) => {
            let pairs: Vec<Value> = m
                .iter()
                .map(|(k, v)| list_from_vec(vec![k.clone(), v.clone()]))
                .collect();
            Ok(list_from_vec(pairs))
        }
        _ => Err(Error::type_error("map", args[0].type_name())),
    }
}

pub fn map_values(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("Map/map-values", 2, args.len()));
    }
    let func = &args[0];
    match &args[1] {
        Value::Map(m) => {
            let mut result = PersistentMap::new();
            for (k, v) in m.iter() {
                let new_v = apply(func, std::slice::from_ref(v))?;
                result.insert(k.clone(), new_v);
            }
            Ok(Value::Map(Rc::new(result)))
        }
        _ => Err(Error::type_error("map", args[1].type_name())),
    }
}

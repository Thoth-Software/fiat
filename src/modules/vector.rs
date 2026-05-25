use std::rc::Rc;

use crate::error::Error;
use crate::value::{Value, list_from_vec};

pub fn append(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("Vector/append", 2, args.len()));
    }
    match &args[0] {
        Value::Vector(v) => {
            let mut updated = (**v).clone();
            updated.push_back(args[1].clone());
            Ok(Value::Vector(Rc::new(updated)))
        }
        _ => Err(Error::type_error("vector", args[0].type_name())),
    }
}

pub fn nth(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("Vector/nth", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::Vector(v), Value::Int(i)) => {
            let idx = usize::try_from(*i)
                .map_err(|_| Error::runtime(format!("Vector/nth: index out of bounds: {i}")))?;
            v.get(idx)
                .cloned()
                .ok_or_else(|| Error::runtime(format!("Vector/nth: index out of bounds: {i}")))
        }
        (Value::Vector(_), _) => Err(Error::type_error("int", args[1].type_name())),
        _ => Err(Error::type_error("vector", args[0].type_name())),
    }
}

pub fn to_list(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Vector/to-list", 1, args.len()));
    }
    match &args[0] {
        Value::Vector(v) => {
            let items: Vec<Value> = v.iter().cloned().collect();
            Ok(list_from_vec(items))
        }
        _ => Err(Error::type_error("vector", args[0].type_name())),
    }
}

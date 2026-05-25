use std::rc::Rc;

use crate::error::Error;
use crate::value::{Value, list_from_vec, list_to_vec};

pub fn downcase(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("String/downcase", 1, args.len()));
    }
    match &args[0] {
        Value::String(s) => Ok(Value::String(Rc::from(s.to_lowercase().as_str()))),
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

pub fn upcase(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("String/upcase", 1, args.len()));
    }
    match &args[0] {
        Value::String(s) => Ok(Value::String(Rc::from(s.to_uppercase().as_str()))),
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

pub fn trim(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("String/trim", 1, args.len()));
    }
    match &args[0] {
        Value::String(s) => Ok(Value::String(Rc::from(s.trim()))),
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

pub fn replace(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 3 {
        return Err(Error::arity("String/replace", 3, args.len()));
    }
    match (&args[0], &args[1], &args[2]) {
        (Value::String(s), Value::String(from), Value::String(to)) => {
            Ok(Value::String(Rc::from(s.replace(&**from, to).as_str())))
        }
        (Value::String(_), Value::String(_), _) => {
            Err(Error::type_error("string", args[2].type_name()))
        }
        (Value::String(_), _, _) => Err(Error::type_error("string", args[1].type_name())),
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

pub fn split(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("String/split", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::String(s), Value::String(sep)) => {
            let parts: Vec<Value> = s
                .split(&**sep)
                .map(|part| Value::String(Rc::from(part)))
                .collect();
            Ok(list_from_vec(parts))
        }
        (Value::String(_), _) => Err(Error::type_error("string", args[1].type_name())),
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

pub fn join(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("String/join", 2, args.len()));
    }
    let Value::String(sep) = &args[0] else {
        return Err(Error::type_error("string", args[0].type_name()));
    };
    let items = collect_strings(&args[1])?;
    Ok(Value::String(Rc::from(items.join(&**sep).as_str())))
}

pub fn concat(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("String/concat", 1, args.len()));
    }
    let items = collect_display_values(&args[0])?;
    let mut result = String::new();
    for item in &items {
        result.push_str(item);
    }
    Ok(Value::String(Rc::from(result.as_str())))
}

pub fn length(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("String/length", 1, args.len()));
    }
    match &args[0] {
        Value::String(s) => {
            let count =
                i64::try_from(s.chars().count()).map_err(|_| Error::runtime("string too long"))?;
            Ok(Value::Int(count))
        }
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

pub fn starts_with(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("String/starts-with?", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::String(s), Value::String(prefix)) => Ok(Value::Bool(s.starts_with(&**prefix))),
        (Value::String(_), _) => Err(Error::type_error("string", args[1].type_name())),
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

fn collect_strings(val: &Value) -> Result<Vec<String>, Error> {
    match val {
        Value::Nil => Ok(Vec::new()),
        Value::List(_) => {
            let items = list_to_vec(val);
            items
                .iter()
                .map(|v| match v {
                    Value::String(s) => Ok(s.to_string()),
                    _ => Err(Error::type_error("string", v.type_name())),
                })
                .collect()
        }
        Value::Vector(v) => v
            .iter()
            .map(|item| match item {
                Value::String(s) => Ok(s.to_string()),
                _ => Err(Error::type_error("string", item.type_name())),
            })
            .collect(),
        _ => Err(Error::type_error("list or vector", val.type_name())),
    }
}

fn collect_display_values(val: &Value) -> Result<Vec<String>, Error> {
    match val {
        Value::Nil => Ok(Vec::new()),
        Value::List(_) => {
            let items = list_to_vec(val);
            Ok(items.iter().map(display_value).collect())
        }
        Value::Vector(v) => Ok(v.iter().map(display_value).collect()),
        _ => Err(Error::type_error("list or vector", val.type_name())),
    }
}

fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string(),
        other => other.to_string(),
    }
}

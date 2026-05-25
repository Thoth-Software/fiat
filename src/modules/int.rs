use std::rc::Rc;

use im_rc::HashMap as PersistentMap;

use crate::error::Error;
use crate::value::{InternedSymbol, Value};

pub fn to_string(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Int/to-string", 1, args.len()));
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::String(Rc::from(n.to_string().as_str()))),
        _ => Err(Error::type_error("int", args[0].type_name())),
    }
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

/// `(Int/parse s)` → `{:ok n}` for a valid integer string or `{:err reason}`
/// for invalid input (non-numeric, empty, or out of `i64` range).
pub fn parse(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Int/parse", 1, args.len()));
    }
    let Value::String(s) = &args[0] else {
        return Err(Error::type_error("string", args[0].type_name()));
    };
    match s.parse::<i64>() {
        Ok(n) => Ok(ok(Value::Int(n))),
        Err(e) => Ok(err(&format!("cannot parse \"{s}\" as integer: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_string_positive() {
        let result = to_string(&[Value::Int(42)]).expect("to-string failed");
        assert_eq!(result, Value::String(Rc::from("42")));
    }

    #[test]
    fn to_string_negative() {
        let result = to_string(&[Value::Int(-7)]).expect("to-string failed");
        assert_eq!(result, Value::String(Rc::from("-7")));
    }

    #[test]
    fn to_string_type_error() {
        assert!(to_string(&[Value::Float(1.0)]).is_err());
    }

    fn kw(name: &str) -> Value {
        Value::Keyword(InternedSymbol::new(name))
    }

    #[test]
    fn parse_valid() {
        let result = parse(&[Value::String(Rc::from("42"))]).expect("parse");
        let Value::Map(m) = &result else {
            panic!("expected map");
        };
        assert_eq!(m.get(&kw("ok")), Some(&Value::Int(42)));
    }

    #[test]
    fn parse_negative() {
        let result = parse(&[Value::String(Rc::from("-7"))]).expect("parse");
        let Value::Map(m) = &result else {
            panic!("expected map");
        };
        assert_eq!(m.get(&kw("ok")), Some(&Value::Int(-7)));
    }

    #[test]
    fn parse_non_numeric_is_err() {
        let result = parse(&[Value::String(Rc::from("abc"))]).expect("parse");
        let Value::Map(m) = &result else {
            panic!("expected map");
        };
        assert!(m.contains_key(&kw("err")));
        assert!(!m.contains_key(&kw("ok")));
    }

    #[test]
    fn parse_empty_is_err() {
        let result = parse(&[Value::String(Rc::from(""))]).expect("parse");
        let Value::Map(m) = &result else {
            panic!("expected map");
        };
        assert!(m.contains_key(&kw("err")));
    }

    #[test]
    fn parse_overflow_is_err() {
        let result = parse(&[Value::String(Rc::from("99999999999999999999999"))]).expect("parse");
        let Value::Map(m) = &result else {
            panic!("expected map");
        };
        assert!(m.contains_key(&kw("err")));
    }

    #[test]
    fn parse_type_error_on_non_string() {
        assert!(parse(&[Value::Int(1)]).is_err());
    }
}

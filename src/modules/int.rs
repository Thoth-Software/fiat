use std::rc::Rc;

use crate::error::Error;
use crate::value::Value;

pub fn to_string(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Int/to-string", 1, args.len()));
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::String(Rc::from(n.to_string().as_str()))),
        _ => Err(Error::type_error("int", args[0].type_name())),
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
}

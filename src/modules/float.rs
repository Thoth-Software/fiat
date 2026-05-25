use std::rc::Rc;

use crate::error::Error;
use crate::value::Value;

pub fn to_string(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Float/to-string", 1, args.len()));
    }
    match &args[0] {
        Value::Float(n) => {
            // Match the float Display formatting in src/value.rs: whole,
            // finite floats render with a trailing ".0".
            let s = if n.fract() == 0.0 && n.is_finite() {
                format!("{n:.1}")
            } else {
                format!("{n}")
            };
            Ok(Value::String(Rc::from(s.as_str())))
        }
        _ => Err(Error::type_error("float", args[0].type_name())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_string_whole() {
        let result = to_string(&[Value::Float(3.0)]).expect("to-string failed");
        assert_eq!(result, Value::String(Rc::from("3.0")));
    }

    #[test]
    fn to_string_fractional() {
        let result = to_string(&[Value::Float(1.5)]).expect("to-string failed");
        assert_eq!(result, Value::String(Rc::from("1.5")));
    }

    #[test]
    fn to_string_type_error() {
        assert!(to_string(&[Value::Int(1)]).is_err());
    }
}

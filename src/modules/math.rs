use crate::error::Error;
use crate::value::Value;

pub fn sqrt(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("Math/sqrt", 1, args.len()));
    }
    #[allow(clippy::cast_precision_loss)]
    let x = match &args[0] {
        Value::Int(n) => *n as f64,
        Value::Float(n) => *n,
        _ => return Err(Error::type_error("number", args[0].type_name())),
    };
    Ok(Value::Float(x.sqrt()))
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    #[test]
    fn sqrt_of_int() {
        let result = sqrt(&[Value::Int(9)]).expect("sqrt failed");
        assert_eq!(result, Value::Float(3.0));
    }

    #[test]
    fn sqrt_of_float() {
        let result = sqrt(&[Value::Float(2.25)]).expect("sqrt failed");
        assert_eq!(result, Value::Float(1.5));
    }

    #[test]
    fn sqrt_type_error() {
        assert!(sqrt(&[Value::String(Rc::from("x"))]).is_err());
    }

    #[test]
    fn sqrt_arity_error() {
        assert!(sqrt(&[]).is_err());
    }
}

use std::rc::Rc;

use crate::env::Env;
use crate::error::Error;
use crate::value::{Cons, Function, InternedSymbol, Value, list_to_vec};

pub fn eval_program(forms: &[Value], env: &Rc<Env>) -> Result<Value, Error> {
    let mut result = Value::Nil;
    for form in forms {
        let desugared = crate::desugar::desugar(form);
        result = eval(&desugared, env)?;
    }
    Ok(result)
}

pub fn eval(expr: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    match expr {
        Value::Nil
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Keyword(_)
        | Value::Function(_) => Ok(expr.clone()),

        Value::Symbol(sym) => env.get(sym),

        Value::List(cons) => eval_list(cons, env),
    }
}

fn eval_list(cons: &Cons, env: &Rc<Env>) -> Result<Value, Error> {
    if let Some(sym) = cons.head.as_symbol() {
        match sym.name() {
            "behold" => return eval_behold(&cons.tail),
            "choose" => return eval_choose(&cons.tail, env),
            "fiat" => return eval_fiat(&cons.tail, env),
            "atom?" => return eval_atom_q(&cons.tail, env),
            "is?" => return eval_is_q(&cons.tail, env),
            "first" => return eval_first(&cons.tail, env),
            "rest" => return eval_rest(&cons.tail, env),
            "bind" => return eval_bind(&cons.tail, env),
            name => {
                if let Some(builtin) = lookup_builtin(name) {
                    let args = eval_args(&cons.tail, env)?;
                    return builtin(&args);
                }
            }
        }
    }

    let func = eval(&cons.head, env)?;
    let args = eval_args(&cons.tail, env)?;
    apply(&func, &args)
}

fn eval_args(tail: &Value, env: &Rc<Env>) -> Result<Vec<Value>, Error> {
    let mut args = Vec::new();
    let mut current = tail;
    loop {
        match current {
            Value::Nil => return Ok(args),
            Value::List(cons) => {
                args.push(eval(&cons.head, env)?);
                current = &cons.tail;
            }
            _ => return Err(Error::runtime("improper argument list")),
        }
    }
}

fn apply(func: &Value, args: &[Value]) -> Result<Value, Error> {
    match func {
        Value::Function(f) => {
            if f.params.len() != args.len() {
                return Err(Error::arity(
                    f.name.as_ref().map_or("<anonymous>", |n| n.name()),
                    f.params.len(),
                    args.len(),
                ));
            }
            let local_env = Env::with_parent(Rc::clone(&f.env));
            for (param, arg) in f.params.iter().zip(args.iter()) {
                local_env.set(param.clone(), arg.clone());
            }
            let mut result = Value::Nil;
            for body_form in &f.body {
                result = eval(body_form, &local_env)?;
            }
            Ok(result)
        }
        _ => Err(Error::not_callable(func.type_name())),
    }
}

// --- Special Forms ---

fn eval_behold(tail: &Value) -> Result<Value, Error> {
    match tail {
        Value::List(cons) if matches!(cons.tail, Value::Nil) => Ok(cons.head.clone()),
        _ => Err(Error::malformed("behold", "expects exactly 1 argument")),
    }
}

fn eval_choose(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let mut current = tail;
    loop {
        match current {
            Value::Nil => return Ok(Value::Nil),
            Value::List(cons) => match &cons.head {
                Value::List(pair) => {
                    let test = eval(&pair.head, env)?;
                    if test.is_truthy() {
                        let result_forms = list_to_vec(&pair.tail);
                        if result_forms.len() != 1 {
                            return Err(Error::malformed(
                                "choose",
                                "each clause must have exactly 2 elements (test result)",
                            ));
                        }
                        return eval(&result_forms[0], env);
                    }
                    current = &cons.tail;
                }
                _ => {
                    return Err(Error::malformed(
                        "choose",
                        "each clause must be a list of (test result)",
                    ));
                }
            },
            _ => return Err(Error::malformed("choose", "expected list of clauses")),
        }
    }
}

fn eval_fiat(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let args = list_to_vec(tail);

    if args.len() == 1
        && let Some(sym) = args[0].as_symbol()
    {
        let first_char = sym.name().chars().next().unwrap_or('a');
        if first_char.is_uppercase() {
            return Err(Error::runtime(format!(
                "module import not yet implemented: {}",
                sym.name()
            )));
        }
    }

    if args.len() < 2 {
        return Err(Error::malformed(
            "fiat",
            "expected (fiat name (params) body...) or (fiat () (params) body...)",
        ));
    }

    let name_val = &args[0];
    let params_val = &args[1];
    let body = args[2..].to_vec();

    if body.is_empty() {
        return Err(Error::malformed("fiat", "function body is empty"));
    }

    let params = parse_params(params_val)?;

    match name_val {
        Value::Nil => {
            let func = Value::Function(Rc::new(Function {
                name: None,
                params,
                body,
                env: Rc::clone(env),
            }));
            Ok(func)
        }
        Value::Symbol(sym) => {
            let func = Rc::new(Function {
                name: Some(sym.clone()),
                params,
                body,
                env: Rc::clone(env),
            });
            let func_val = Value::Function(func);
            env.set(sym.clone(), func_val.clone());
            Ok(func_val)
        }
        _ => Err(Error::malformed(
            "fiat",
            "name must be a symbol or () for anonymous",
        )),
    }
}

fn parse_params(val: &Value) -> Result<Vec<InternedSymbol>, Error> {
    let items = list_to_vec(val);
    let mut params = Vec::new();
    for item in &items {
        match item {
            Value::Symbol(s) => params.push(s.clone()),
            _ => return Err(Error::malformed("fiat", "parameters must be symbols")),
        }
    }
    Ok(params)
}

fn eval_atom_q(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let args = eval_args(tail, env)?;
    if args.len() != 1 {
        return Err(Error::arity("atom?", 1, args.len()));
    }
    Ok(Value::Bool(args[0].is_atom()))
}

fn eval_is_q(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let args = eval_args(tail, env)?;
    if args.len() != 2 {
        return Err(Error::arity("is?", 2, args.len()));
    }
    let a = &args[0];
    let b = &args[1];

    if !a.is_atom() {
        return Err(Error::is_on_collection(a.type_name()));
    }
    if !b.is_atom() {
        return Err(Error::is_on_collection(b.type_name()));
    }

    #[allow(clippy::float_cmp)]
    let result = match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Symbol(x), Value::Symbol(y)) | (Value::Keyword(x), Value::Keyword(y)) => {
            x.ptr_eq(y)
        }
        (Value::String(x), Value::String(y)) => Rc::ptr_eq(x, y),
        _ => false,
    };
    Ok(Value::Bool(result))
}

fn eval_first(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let args = eval_args(tail, env)?;
    if args.len() != 1 {
        return Err(Error::arity("first", 1, args.len()));
    }
    match &args[0] {
        Value::List(cell) => Ok(cell.head.clone()),
        Value::Nil => Err(Error::first_on_empty_list()),
        _ => Err(Error::type_error("list", args[0].type_name())),
    }
}

fn eval_rest(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let args = eval_args(tail, env)?;
    if args.len() != 1 {
        return Err(Error::arity("rest", 1, args.len()));
    }
    match &args[0] {
        Value::List(cell) => Ok(cell.tail.clone()),
        Value::Nil => Ok(Value::Nil),
        _ => Err(Error::type_error("list", args[0].type_name())),
    }
}

fn eval_bind(tail: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    let args = eval_args(tail, env)?;
    if args.len() != 2 {
        return Err(Error::arity("bind", 2, args.len()));
    }
    let val = args[0].clone();
    let list = &args[1];
    match list {
        Value::Nil | Value::List(_) => Ok(Value::List(Rc::new(Cons {
            head: val,
            tail: list.clone(),
        }))),
        _ => Err(Error::bind_non_list(list.type_name())),
    }
}

// --- Built-in Arithmetic ---

type BuiltinFn = fn(&[Value]) -> Result<Value, Error>;

fn lookup_builtin(name: &str) -> Option<BuiltinFn> {
    match name {
        "+" => Some(builtin_add),
        "-" => Some(builtin_sub),
        "*" => Some(builtin_mul),
        "/" => Some(builtin_div),
        "%" => Some(builtin_rem),
        ">" => Some(builtin_gt),
        "<" => Some(builtin_lt),
        "=" => Some(builtin_eq),
        _ => None,
    }
}

fn numeric_binop<FI, FF>(
    name: &str,
    args: &[Value],
    int_op: FI,
    float_op: FF,
) -> Result<Value, Error>
where
    FI: FnOnce(i64, i64) -> Result<Value, Error>,
    FF: FnOnce(f64, f64) -> Result<Value, Error>,
{
    if args.len() != 2 {
        return Err(Error::arity(name, 2, args.len()));
    }
    #[allow(clippy::cast_precision_loss)]
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => int_op(*a, *b),
        (Value::Float(a), Value::Float(b)) => float_op(*a, *b),
        (Value::Int(a), Value::Float(b)) => float_op(*a as f64, *b),
        (Value::Float(a), Value::Int(b)) => float_op(*a, *b as f64),
        _ => Err(Error::type_error(
            "number",
            &format!("({}, {})", args[0].type_name(), args[1].type_name()),
        )),
    }
}

fn builtin_add(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "+",
        args,
        |a, b| Ok(Value::Int(a + b)),
        |a, b| Ok(Value::Float(a + b)),
    )
}

fn builtin_sub(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "-",
        args,
        |a, b| Ok(Value::Int(a - b)),
        |a, b| Ok(Value::Float(a - b)),
    )
}

fn builtin_mul(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "*",
        args,
        |a, b| Ok(Value::Int(a * b)),
        |a, b| Ok(Value::Float(a * b)),
    )
}

fn builtin_div(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "/",
        args,
        |a, b| {
            if b == 0 {
                Err(Error::division_by_zero())
            } else {
                Ok(Value::Int(a / b))
            }
        },
        |a, b| {
            if b == 0.0 {
                Err(Error::division_by_zero())
            } else {
                Ok(Value::Float(a / b))
            }
        },
    )
}

fn builtin_rem(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "%",
        args,
        |a, b| {
            if b == 0 {
                Err(Error::division_by_zero())
            } else {
                Ok(Value::Int(a % b))
            }
        },
        |a, b| {
            if b == 0.0 {
                Err(Error::division_by_zero())
            } else {
                Ok(Value::Float(a % b))
            }
        },
    )
}

fn builtin_gt(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        ">",
        args,
        |a, b| Ok(Value::Bool(a > b)),
        |a, b| Ok(Value::Bool(a > b)),
    )
}

fn builtin_lt(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "<",
        args,
        |a, b| Ok(Value::Bool(a < b)),
        |a, b| Ok(Value::Bool(a < b)),
    )
}

fn builtin_eq(args: &[Value]) -> Result<Value, Error> {
    numeric_binop(
        "=",
        args,
        |a, b| Ok(Value::Bool(a == b)),
        |a, b| Ok(Value::Bool(a == b)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::read;

    fn eval_str(source: &str) -> Result<Value, Error> {
        let forms = read(source).map_err(|e| Error::runtime(format!("read error: {e}")))?;
        let env = Env::new();
        eval_program(&forms, &env)
    }

    #[test]
    fn eval_self_evaluating() {
        assert_eq!(eval_str("42").ok(), Some(Value::Int(42)));
        assert_eq!(eval_str("true").ok(), Some(Value::Bool(true)));
        assert_eq!(eval_str("\"hi\"").ok(), Some(Value::String(Rc::from("hi"))));
        assert_eq!(
            eval_str(":ok").ok(),
            Some(Value::Keyword(InternedSymbol::new("ok")))
        );
    }

    #[test]
    fn eval_arithmetic() {
        assert_eq!(eval_str("(+ 1 2)").ok(), Some(Value::Int(3)));
        assert_eq!(eval_str("(- 10 3)").ok(), Some(Value::Int(7)));
        assert_eq!(eval_str("(* 4 5)").ok(), Some(Value::Int(20)));
        assert_eq!(eval_str("(/ 10 3)").ok(), Some(Value::Int(3)));
        assert_eq!(eval_str("(% 10 3)").ok(), Some(Value::Int(1)));
    }

    #[test]
    fn eval_comparison() {
        assert_eq!(eval_str("(> 3 2)").ok(), Some(Value::Bool(true)));
        assert_eq!(eval_str("(< 3 2)").ok(), Some(Value::Bool(false)));
        assert_eq!(eval_str("(= 3 3)").ok(), Some(Value::Bool(true)));
    }

    #[test]
    fn eval_behold() {
        let result = eval_str("(behold (+ 1 2))").expect("should work");
        assert_eq!(result.to_string(), "(+ 1 2)");
    }

    #[test]
    fn eval_quote_shorthand() {
        let result = eval_str("'(a b c)").expect("should work");
        assert_eq!(result.to_string(), "(a b c)");
    }

    #[test]
    fn eval_atom_q() {
        assert_eq!(eval_str("(atom? 42)").ok(), Some(Value::Bool(true)));
        assert_eq!(eval_str("(atom? '(1))").ok(), Some(Value::Bool(false)));
        assert_eq!(eval_str("(atom? ())").ok(), Some(Value::Bool(true)));
    }

    #[test]
    fn eval_is_q() {
        assert_eq!(eval_str("(is? 1 1)").ok(), Some(Value::Bool(true)));
        assert_eq!(eval_str("(is? 1 2)").ok(), Some(Value::Bool(false)));
        assert_eq!(eval_str("(is? 'a 'a)").ok(), Some(Value::Bool(true)));
    }

    #[test]
    fn eval_first_rest_bind() {
        assert_eq!(eval_str("(first '(1 2 3))").ok(), Some(Value::Int(1)));
        assert_eq!(
            eval_str("(rest '(1 2 3))").ok().map(|v| v.to_string()),
            Some("(2 3)".to_string())
        );
        assert_eq!(
            eval_str("(bind 1 '(2 3))").ok().map(|v| v.to_string()),
            Some("(1 2 3)".to_string())
        );
    }

    #[test]
    fn eval_choose() {
        assert_eq!(
            eval_str("(choose ((= 1 2) 'a) (true 'b))").ok(),
            Some(Value::Symbol(InternedSymbol::new("b")))
        );
    }

    #[test]
    fn eval_fiat_anonymous() {
        assert_eq!(
            eval_str("((fiat () (x) (* x 2)) 5)").ok(),
            Some(Value::Int(10))
        );
    }

    #[test]
    fn eval_fiat_named() {
        assert_eq!(
            eval_str("(fiat double (x) (* x 2)) (double 21)").ok(),
            Some(Value::Int(42))
        );
    }

    #[test]
    fn eval_fiat_recursive() {
        let src = "
            (fiat factorial (n)
              (choose
                ((= n 0) 1)
                (true (* n (factorial (- n 1))))))
            (factorial 5)
        ";
        assert_eq!(eval_str(src).ok(), Some(Value::Int(120)));
    }

    #[test]
    fn eval_fiat_multi_body() {
        let src = "
            (fiat outer (x)
              (fiat helper (y) (+ y 1))
              (helper x))
            (outer 10)
        ";
        assert_eq!(eval_str(src).ok(), Some(Value::Int(11)));
    }

    #[test]
    fn eval_division_by_zero() {
        assert!(eval_str("(/ 1 0)").is_err());
    }

    #[test]
    fn eval_is_on_collection_error() {
        assert!(eval_str("(is? '(1) '(1))").is_err());
    }

    #[test]
    fn eval_first_on_nil_error() {
        assert!(eval_str("(first ())").is_err());
    }

    #[test]
    fn eval_bind_non_list_error() {
        assert!(eval_str("(bind 1 2)").is_err());
    }
}

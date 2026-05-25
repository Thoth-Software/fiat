use std::rc::Rc;

use im_rc::{HashMap as PersistentMap, HashSet as PersistentSet, Vector as PersistentVector};

use crate::env::Env;
use crate::error::Error;
use crate::value::{
    Builtin, BuiltinFn, Cons, Function, InternedSymbol, Value, list_from_vec, list_to_vec,
};

pub fn eval_program(forms: &[Value], env: &Rc<Env>) -> Result<Value, Error> {
    let mut result = Value::Nil;
    for form in forms {
        let desugared = crate::desugar::desugar(form);
        result = eval(&desugared, env)?;
    }
    Ok(result)
}

enum Step {
    Done(Value),
    TailCall { func: Value, args: Vec<Value> },
}

pub fn eval(expr: &Value, env: &Rc<Env>) -> Result<Value, Error> {
    match eval_inner(expr, env)? {
        Step::Done(v) => Ok(v),
        Step::TailCall { func, args } => trampoline(&func, &args),
    }
}

fn eval_inner(expr: &Value, env: &Rc<Env>) -> Result<Step, Error> {
    match expr {
        Value::Nil
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Keyword(_)
        | Value::Function(_)
        | Value::Builtin(_) => Ok(Step::Done(expr.clone())),

        Value::Vector(items) => {
            let evaluated: Result<PersistentVector<Value>, Error> =
                items.iter().map(|item| eval(item, env)).collect();
            Ok(Step::Done(Value::Vector(Rc::new(evaluated?))))
        }
        Value::Map(entries) => {
            let evaluated: Result<PersistentMap<Value, Value>, Error> = entries
                .iter()
                .map(|(k, v)| Ok((eval(k, env)?, eval(v, env)?)))
                .collect();
            Ok(Step::Done(Value::Map(Rc::new(evaluated?))))
        }
        Value::Set(items) => {
            let evaluated: Result<PersistentSet<Value>, Error> =
                items.iter().map(|item| eval(item, env)).collect();
            Ok(Step::Done(Value::Set(Rc::new(evaluated?))))
        }

        Value::Symbol(sym) => match env.get(sym) {
            Ok(value) => Ok(Step::Done(value)),
            Err(err) => builtin_value(sym.name()).map(Step::Done).ok_or(err),
        },

        Value::List(cons) => eval_list_tail(cons, env),
    }
}

fn eval_list_tail(cons: &Cons, env: &Rc<Env>) -> Result<Step, Error> {
    if let Some(sym) = cons.head.as_symbol() {
        match sym.name() {
            "behold" => return eval_behold(&cons.tail).map(Step::Done),
            "choose" => return eval_choose_tail(&cons.tail, env),
            "fiat" => return eval_fiat(&cons.tail, env).map(Step::Done),
            "atom?" => return eval_atom_q(&cons.tail, env).map(Step::Done),
            "is?" => return eval_is_q(&cons.tail, env).map(Step::Done),
            "first" => return eval_first(&cons.tail, env).map(Step::Done),
            "rest" => return eval_rest(&cons.tail, env).map(Step::Done),
            "bind" => return eval_bind(&cons.tail, env).map(Step::Done),
            name => {
                if let Some(builtin) = lookup_builtin(name) {
                    let args = eval_args(&cons.tail, env)?;
                    return builtin(&args).map(Step::Done);
                }
            }
        }
    }

    let func = eval(&cons.head, env)?;
    let args = eval_args(&cons.tail, env)?;
    Ok(Step::TailCall { func, args })
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

pub fn apply(func: &Value, args: &[Value]) -> Result<Value, Error> {
    trampoline(func, args)
}

fn trampoline(func: &Value, args: &[Value]) -> Result<Value, Error> {
    let mut step = apply_once(func, args)?;
    loop {
        match step {
            Step::Done(v) => return Ok(v),
            Step::TailCall { func, args } => {
                step = apply_once(&func, &args)?;
            }
        }
    }
}

fn apply_once(func: &Value, args: &[Value]) -> Result<Step, Error> {
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
            let body_len = f.body.len();
            for body_form in &f.body[..body_len - 1] {
                eval(body_form, &local_env)?;
            }
            eval_inner(&f.body[body_len - 1], &local_env)
        }
        Value::Builtin(b) => (b.func)(args).map(Step::Done),
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

fn eval_choose_tail(tail: &Value, env: &Rc<Env>) -> Result<Step, Error> {
    let mut current = tail;
    loop {
        match current {
            Value::Nil => return Ok(Step::Done(Value::Nil)),
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
                        return eval_inner(&result_forms[0], env);
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
            return crate::modules::import_module(sym.name(), env);
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

// --- Built-in Arithmetic & Set Operations ---

fn builtin_entry(name: &str) -> Option<(&'static str, BuiltinFn)> {
    Some(match name {
        "+" => ("+", builtin_add),
        "-" => ("-", builtin_sub),
        "*" => ("*", builtin_mul),
        "/" => ("/", builtin_div),
        "%" => ("%", builtin_rem),
        ">" => (">", builtin_gt),
        "<" => ("<", builtin_lt),
        "=" => ("=", builtin_eq),
        "set?" => ("set?", builtin_set_q),
        "has?" => ("has?", builtin_has_q),
        "union" => ("union", builtin_union),
        "intersect" => ("intersect", builtin_intersect),
        "without" => ("without", builtin_without),
        "as-codepoints" => ("as-codepoints", builtin_as_codepoints),
        "as-graphemes" => ("as-graphemes", builtin_as_graphemes),
        "as-bytes" => ("as-bytes", builtin_as_bytes),
        "from-codepoints" => ("from-codepoints", builtin_from_codepoints),
        _ => return None,
    })
}

fn lookup_builtin(name: &str) -> Option<BuiltinFn> {
    builtin_entry(name).map(|(_, func)| func)
}

fn builtin_value(name: &str) -> Option<Value> {
    builtin_entry(name).map(|(sname, func)| Value::Builtin(Builtin { name: sname, func }))
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

// --- Set Operations ---

fn builtin_set_q(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("set?", 1, args.len()));
    }
    Ok(Value::Bool(matches!(args[0], Value::Set(_))))
}

fn builtin_has_q(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("has?", 2, args.len()));
    }
    match &args[1] {
        Value::Set(s) => Ok(Value::Bool(s.contains(&args[0]))),
        _ => Err(Error::type_error("set", args[1].type_name())),
    }
}

fn builtin_union(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("union", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::Set(a), Value::Set(b)) => {
            Ok(Value::Set(Rc::new((**a).clone().union((**b).clone()))))
        }
        (Value::Set(_), _) => Err(Error::type_error("set", args[1].type_name())),
        _ => Err(Error::type_error("set", args[0].type_name())),
    }
}

fn builtin_intersect(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("intersect", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::Set(a), Value::Set(b)) => Ok(Value::Set(Rc::new(
            (**a).clone().intersection((**b).clone()),
        ))),
        (Value::Set(_), _) => Err(Error::type_error("set", args[1].type_name())),
        _ => Err(Error::type_error("set", args[0].type_name())),
    }
}

fn builtin_without(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity("without", 2, args.len()));
    }
    match (&args[0], &args[1]) {
        (Value::Set(a), Value::Set(b)) => {
            Ok(Value::Set(Rc::new((**a).clone().difference((**b).clone()))))
        }
        (Value::Set(_), _) => Err(Error::type_error("set", args[1].type_name())),
        _ => Err(Error::type_error("set", args[0].type_name())),
    }
}

// --- String Decomposition ---

fn builtin_as_codepoints(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("as-codepoints", 1, args.len()));
    }
    match &args[0] {
        Value::String(s) => {
            let cps: Vec<Value> = s.chars().map(|c| Value::Int(i64::from(c as u32))).collect();
            Ok(list_from_vec(cps))
        }
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

fn builtin_as_graphemes(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("as-graphemes", 1, args.len()));
    }
    match &args[0] {
        // char-based segmentation (one Rust char = one Unicode scalar value)
        Value::String(s) => {
            let gs: Vec<Value> = s
                .chars()
                .map(|c| {
                    let mut buf = [0u8; 4];
                    Value::String(Rc::from(c.encode_utf8(&mut buf) as &str))
                })
                .collect();
            Ok(list_from_vec(gs))
        }
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

fn builtin_as_bytes(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("as-bytes", 1, args.len()));
    }
    match &args[0] {
        Value::String(s) => {
            let bs: Vec<Value> = s
                .as_bytes()
                .iter()
                .map(|&b| Value::Int(i64::from(b)))
                .collect();
            Ok(list_from_vec(bs))
        }
        _ => Err(Error::type_error("string", args[0].type_name())),
    }
}

fn builtin_from_codepoints(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity("from-codepoints", 1, args.len()));
    }
    let items = list_to_vec(&args[0]);
    let mut result = String::new();
    for item in &items {
        match item {
            Value::Int(n) => {
                let cp = u32::try_from(*n)
                    .map_err(|_| Error::runtime(format!("invalid codepoint: {n}")))?;
                let c = char::from_u32(cp)
                    .ok_or_else(|| Error::runtime(format!("invalid codepoint: {n}")))?;
                result.push(c);
            }
            _ => return Err(Error::type_error("int", item.type_name())),
        }
    }
    Ok(Value::String(Rc::from(result.as_str())))
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

    #[test]
    fn builtin_is_first_class_value() {
        let result = eval_str("+").expect("should resolve to a builtin");
        assert_eq!(result.to_string(), "<builtin +>");
    }

    #[test]
    fn builtin_passed_as_argument() {
        // A primitive passed into a closure and applied there.
        assert_eq!(
            eval_str("((fiat () (f) (f 3 4)) +)").ok(),
            Some(Value::Int(7))
        );
    }

    #[test]
    fn unbound_non_builtin_symbol_errors() {
        assert!(eval_str("nope").is_err());
    }

    #[test]
    fn is_q_errors_on_collection_value() {
        let env = Env::new();
        let empty_set = Value::Set(Rc::new(im_rc::HashSet::new()));
        env.set(InternedSymbol::new("s"), empty_set);
        let forms = read("(is? s s)").expect("read error");
        assert!(eval_program(&forms, &env).is_err());
    }

    #[test]
    fn eval_vector_literal() {
        let src = "(let ((x 5)) [x (+ x 1) (+ x 2)])";
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some("[5 6 7]".to_string())
        );
    }

    #[test]
    fn eval_map_literal() {
        let src = "{:x (+ 1 2) :y (* 3 4)}";
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        assert!(s.contains(":x 3"));
        assert!(s.contains(":y 12"));
    }

    #[test]
    fn eval_set_literal() {
        let src = "#{(+ 1 2) (* 3 4)}";
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        assert!(s.contains('3'));
        assert!(s.contains("12"));
    }

    #[test]
    fn behold_vector_is_inert() {
        let src = "(let ((x 99)) (behold [x (+ x 1)]))";
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        assert!(s.contains('x'));
        assert!(s.contains("(+ x 1)"));
    }

    #[test]
    fn behold_map_is_inert() {
        let result = eval_str("(behold {:k (+ 1 2)})").expect("should eval");
        assert_eq!(result.to_string(), "{:k (+ 1 2)}");
    }

    #[test]
    fn eval_nested_collection() {
        let src = "(let ((x 1)) [{:a x} #{x (+ x 1)}])";
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        assert!(s.starts_with("[{:a 1}"));
    }

    #[test]
    fn set_q_true_for_set() {
        assert_eq!(eval_str("(set? #{1 2})").ok(), Some(Value::Bool(true)));
    }

    #[test]
    fn set_q_false_for_list() {
        assert_eq!(eval_str("(set? '(1 2))").ok(), Some(Value::Bool(false)));
    }

    #[test]
    fn has_q_found() {
        assert_eq!(
            eval_str("(has? 'a #{'a 'b 'c})").ok(),
            Some(Value::Bool(true))
        );
    }

    #[test]
    fn has_q_not_found() {
        assert_eq!(
            eval_str("(has? 'z #{'a 'b})").ok(),
            Some(Value::Bool(false))
        );
    }

    #[test]
    fn has_q_type_error() {
        assert!(eval_str("(has? 1 '(1 2))").is_err());
    }

    #[test]
    fn union_sets() {
        let result = eval_str("(union #{'a 'b} #{'b 'c})").expect("should eval");
        if let Value::Set(s) = &result {
            assert_eq!(s.len(), 3);
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("a"))));
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("b"))));
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("c"))));
        } else {
            panic!("expected set");
        }
    }

    #[test]
    fn intersect_sets() {
        let result = eval_str("(intersect #{'a 'b 'c} #{'b 'c 'd})").expect("should eval");
        if let Value::Set(s) = &result {
            assert_eq!(s.len(), 2);
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("b"))));
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("c"))));
        } else {
            panic!("expected set");
        }
    }

    #[test]
    fn without_sets() {
        let result = eval_str("(without #{'a 'b 'c} #{'b})").expect("should eval");
        if let Value::Set(s) = &result {
            assert_eq!(s.len(), 2);
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("a"))));
            assert!(s.contains(&Value::Symbol(InternedSymbol::new("c"))));
        } else {
            panic!("expected set");
        }
    }

    #[test]
    fn union_type_error() {
        assert!(eval_str("(union #{'a} '(b))").is_err());
    }

    #[test]
    fn set_ops_are_first_class() {
        assert_eq!(
            eval_str("set?").ok().map(|v| v.to_string()),
            Some("<builtin set?>".to_string())
        );
    }

    #[test]
    fn fiat_lux_succeeds() {
        assert_eq!(eval_str("(fiat Lux)").ok(), Some(Value::Nil));
    }

    #[test]
    fn unknown_module_errors() {
        assert!(eval_str("(fiat Bogus)").is_err());
    }

    #[test]
    fn namespaced_call_after_import() {
        assert_eq!(
            eval_str("(fiat Lux) (Int/to-string 42)").ok(),
            Some(Value::String(Rc::from("42")))
        );
    }

    #[test]
    fn namespaced_call_without_import_errors() {
        assert!(eval_str("(Int/to-string 42)").is_err());
    }

    #[test]
    fn math_sqrt_of_int() {
        assert_eq!(
            eval_str("(fiat Lux) (Math/sqrt 9)").ok(),
            Some(Value::Float(3.0))
        );
    }

    #[test]
    fn math_sqrt_of_float() {
        assert_eq!(
            eval_str("(fiat Lux) (Math/sqrt 2.25)").ok(),
            Some(Value::Float(1.5))
        );
    }

    #[test]
    fn float_to_string_whole() {
        assert_eq!(
            eval_str("(fiat Lux) (Float/to-string 3.0)").ok(),
            Some(Value::String(Rc::from("3.0")))
        );
    }

    #[test]
    fn float_to_string_fractional() {
        assert_eq!(
            eval_str("(fiat Lux) (Float/to-string 1.5)").ok(),
            Some(Value::String(Rc::from("1.5")))
        );
    }

    #[test]
    fn namespaced_builtin_is_first_class() {
        let src = r#"
            (fiat Lux)
            (let ((f Int/to-string))
              (f 99))
        "#;
        assert_eq!(eval_str(src).ok(), Some(Value::String(Rc::from("99"))));
    }

    #[test]
    fn map_get_found() {
        let src = "(fiat Lux) (Map/get {:a 1 :b 2} :a 0)";
        assert_eq!(eval_str(src).ok(), Some(Value::Int(1)));
    }

    #[test]
    fn map_get_default() {
        let src = "(fiat Lux) (Map/get {:a 1} :z 99)";
        assert_eq!(eval_str(src).ok(), Some(Value::Int(99)));
    }

    #[test]
    fn map_put_roundtrip() {
        let src = r#"
            (fiat Lux)
            (let ((m (Map/put {} :count 0)))
              (let ((m2 (Map/put m :count (+ (Map/get m :count 0) 1))))
                (Map/get m2 :count 0)))
        "#;
        assert_eq!(eval_str(src).ok(), Some(Value::Int(1)));
    }

    #[test]
    fn map_merge_override() {
        let src = r#"
            (fiat Lux)
            (let ((a {:x 1 :y 2})
                  (b {:y 3 :z 4}))
              (Map/get (Map/merge a b) :y 0))
        "#;
        assert_eq!(eval_str(src).ok(), Some(Value::Int(3)));
    }

    #[test]
    fn map_entries_shape() {
        let src = "(fiat Lux) (Map/entries {:a 1})";
        let result = eval_str(src).expect("should eval");
        assert_eq!(result.to_string(), "((:a 1))");
    }

    #[test]
    fn map_map_values() {
        let src = r#"
            (fiat Lux)
            (Map/get
              (Map/map-values (fiat () (v) (* v 10)) {:a 1 :b 2})
              :a 0)
        "#;
        assert_eq!(eval_str(src).ok(), Some(Value::Int(10)));
    }

    #[test]
    fn map_get_type_error() {
        assert!(eval_str("(fiat Lux) (Map/get '(1 2) :a 0)").is_err());
    }

    #[test]
    fn vector_append() {
        let src = "(fiat Lux) (Vector/append [1 2] 3)";
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some("[1 2 3]".to_string())
        );
    }

    #[test]
    fn vector_append_immutability() {
        let src = r#"
            (fiat Lux)
            (let ((v [1 2]))
              (let ((_ (Vector/append v 3)))
                v))
        "#;
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some("[1 2]".to_string())
        );
    }

    #[test]
    fn vector_nth() {
        let src = "(fiat Lux) (Vector/nth [10 20 30] 1)";
        assert_eq!(eval_str(src).ok(), Some(Value::Int(20)));
    }

    #[test]
    fn vector_nth_out_of_bounds() {
        assert!(eval_str("(fiat Lux) (Vector/nth [1 2] 5)").is_err());
    }

    #[test]
    fn vector_to_list_order() {
        let src = "(fiat Lux) (Vector/to-list [1 2 3])";
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some("(1 2 3)".to_string())
        );
    }

    #[test]
    fn vector_to_list_empty() {
        let src = "(fiat Lux) (Vector/to-list [])";
        assert_eq!(eval_str(src).ok(), Some(Value::Nil));
    }

    #[test]
    fn string_downcase() {
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/downcase "Hello World")"#).ok(),
            Some(Value::String(Rc::from("hello world")))
        );
    }

    #[test]
    fn string_upcase() {
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/upcase "hello")"#).ok(),
            Some(Value::String(Rc::from("HELLO")))
        );
    }

    #[test]
    fn string_trim() {
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/trim "  hi  ")"#).ok(),
            Some(Value::String(Rc::from("hi")))
        );
    }

    #[test]
    fn string_replace() {
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/replace "a-b-c" "-" "_")"#).ok(),
            Some(Value::String(Rc::from("a_b_c")))
        );
    }

    #[test]
    fn string_split_produces_list() {
        let src = r#"(fiat Lux) (String/split "a,b,c" ",")"#;
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some(r#"("a" "b" "c")"#.to_string())
        );
    }

    #[test]
    fn string_join() {
        let src = r#"(fiat Lux) (String/join ", " '("x" "y" "z"))"#;
        assert_eq!(eval_str(src).ok(), Some(Value::String(Rc::from("x, y, z"))));
    }

    #[test]
    fn string_concat() {
        let src = r#"(fiat Lux) (String/concat '("hello" " " "world"))"#;
        assert_eq!(
            eval_str(src).ok(),
            Some(Value::String(Rc::from("hello world")))
        );
    }

    #[test]
    fn string_length() {
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/length "hello")"#).ok(),
            Some(Value::Int(5))
        );
    }

    #[test]
    fn string_starts_with() {
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/starts-with? "hello" "hel")"#).ok(),
            Some(Value::Bool(true))
        );
        assert_eq!(
            eval_str(r#"(fiat Lux) (String/starts-with? "hello" "xyz")"#).ok(),
            Some(Value::Bool(false))
        );
    }

    #[test]
    fn as_codepoints_ascii() {
        let src = r#"(as-codepoints "Hi")"#;
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some("(72 105)".to_string())
        );
    }

    #[test]
    fn as_codepoints_non_ascii() {
        let src = r#"(as-codepoints "café")"#;
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        // 'é' is U+00E9 = 233
        assert_eq!(s, "(99 97 102 233)");
    }

    #[test]
    fn as_bytes_non_ascii() {
        let src = r#"(as-bytes "café")"#;
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        // 'é' is U+00E9, UTF-8 encoding is 0xC3 0xA9 = 195 169
        assert_eq!(s, "(99 97 102 195 169)");
    }

    #[test]
    fn as_graphemes_ascii() {
        let src = r#"(as-graphemes "ab")"#;
        assert_eq!(
            eval_str(src).ok().map(|v| v.to_string()),
            Some(r#"("a" "b")"#.to_string())
        );
    }

    #[test]
    fn as_graphemes_non_ascii() {
        let src = r#"(as-graphemes "café")"#;
        let result = eval_str(src).expect("should eval");
        let s = result.to_string();
        assert_eq!(s, r#"("c" "a" "f" "é")"#);
    }

    #[test]
    fn from_codepoints_basic() {
        let src = "(from-codepoints '(72 101 108 108 111))";
        assert_eq!(eval_str(src).ok(), Some(Value::String(Rc::from("Hello"))));
    }

    #[test]
    fn from_codepoints_roundtrip() {
        let src = r#"(from-codepoints (as-codepoints "Hello"))"#;
        assert_eq!(eval_str(src).ok(), Some(Value::String(Rc::from("Hello"))));
    }

    #[test]
    fn from_codepoints_roundtrip_non_ascii() {
        let src = r#"(from-codepoints (as-codepoints "café"))"#;
        assert_eq!(eval_str(src).ok(), Some(Value::String(Rc::from("café"))));
    }

    #[test]
    fn from_codepoints_invalid() {
        let src = "(from-codepoints '(1114112))";
        assert!(eval_str(src).is_err());
    }

    #[test]
    fn from_codepoints_type_error() {
        let src = r#"(from-codepoints '("a"))"#;
        assert!(eval_str(src).is_err());
    }

    #[test]
    fn as_codepoints_type_error() {
        assert!(eval_str("(as-codepoints 42)").is_err());
    }

    #[test]
    fn as_bytes_type_error() {
        assert!(eval_str("(as-bytes 42)").is_err());
    }

    #[test]
    fn as_graphemes_type_error() {
        assert!(eval_str("(as-graphemes 42)").is_err());
    }

    #[test]
    fn codepoints_vs_bytes_distinction() {
        // 'é' (U+00E9) is 1 codepoint but 2 bytes in UTF-8
        let cp_src = r#"(as-codepoints "é")"#;
        let by_src = r#"(as-bytes "é")"#;
        let cp = eval_str(cp_src).expect("codepoints");
        let by = eval_str(by_src).expect("bytes");
        assert_eq!(cp.to_string(), "(233)");
        assert_eq!(by.to_string(), "(195 169)");
    }

    fn eval_with_prelude(source: &str) -> Result<Value, Error> {
        let forms = read(source).map_err(|e| Error::runtime(format!("read error: {e}")))?;
        let env = crate::prelude::environment()
            .map_err(|e| Error::runtime(format!("prelude error: {e}")))?;
        eval_program(&forms, &env)
    }

    #[test]
    fn tco_self_recursion_deep() {
        let source = r#"
            (fiat count-down (n)
              (choose
                ((= n 0) 0)
                (true (count-down (- n 1)))))
            (count-down 100000)
        "#;
        assert_eq!(eval_str(source).ok(), Some(Value::Int(0)));
    }

    #[test]
    fn tco_reverse_100k() {
        let source = r#"
            (fiat build (n acc)
              (choose
                ((= n 0) acc)
                (true (build (- n 1) (bind n acc)))))
            (let ((big-list (build 100000 ())))
              (first (reverse big-list)))
        "#;
        let result = eval_with_prelude(source).expect("should not overflow");
        assert_eq!(result, Value::Int(100000));
    }

    #[test]
    fn tco_mutual_recursion() {
        let source = r#"
            (fiat my-even? (n)
              (choose
                ((= n 0) true)
                (true (my-odd? (- n 1)))))
            (fiat my-odd? (n)
              (choose
                ((= n 0) false)
                (true (my-even? (- n 1)))))
            (my-even? 100000)
        "#;
        assert_eq!(eval_str(source).ok(), Some(Value::Bool(true)));
    }

    #[test]
    fn tco_mutual_recursion_odd() {
        let source = r#"
            (fiat my-even? (n)
              (choose
                ((= n 0) true)
                (true (my-odd? (- n 1)))))
            (fiat my-odd? (n)
              (choose
                ((= n 0) false)
                (true (my-even? (- n 1)))))
            (my-odd? 99999)
        "#;
        assert_eq!(eval_str(source).ok(), Some(Value::Bool(true)));
    }
}

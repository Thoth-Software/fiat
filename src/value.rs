use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use im_rc::{HashMap as PersistentMap, HashSet as PersistentSet, Vector as PersistentVector};

use crate::env::Env;
use crate::error::Error;

// --- Symbol Interning ---

#[derive(Debug, Clone)]
pub struct InternedSymbol(Rc<str>);

impl InternedSymbol {
    pub fn new(name: &str) -> Self {
        INTERNER.with_borrow_mut(|map| {
            if let Some(existing) = map.get(name).cloned() {
                return Self(existing);
            }
            let rc: Rc<str> = Rc::from(name);
            map.insert(name.to_string(), Rc::clone(&rc));
            Self(rc)
        })
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for InternedSymbol {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for InternedSymbol {}

impl std::hash::Hash for InternedSymbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl fmt::Display for InternedSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

thread_local! {
    static INTERNER: RefCell<HashMap<String, Rc<str>>> = RefCell::new(HashMap::new());
}

// --- Cons Cell ---

#[derive(Debug, Clone)]
pub struct Cons {
    pub head: Value,
    pub tail: Value,
}

// --- Function ---

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Option<InternedSymbol>,
    pub params: Vec<InternedSymbol>,
    pub body: Vec<Value>,
    pub env: Rc<Env>,
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "<function {name}>"),
            None => write!(f, "<anonymous function>"),
        }
    }
}

// --- Builtin (native Rust function) ---

pub type BuiltinFn = fn(&[Value]) -> Result<Value, Error>;

/// A primitive operation implemented in Rust (arithmetic, set ops, etc.).
/// Carried as a first-class value so it can be passed to higher-order
/// functions like `fold` and `map`.
#[derive(Clone)]
pub struct Builtin {
    pub name: &'static str,
    pub func: BuiltinFn,
}

impl fmt::Debug for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<builtin {}>", self.name)
    }
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<builtin {}>", self.name)
    }
}

// --- Value ---

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<str>),
    Symbol(InternedSymbol),
    Keyword(InternedSymbol),
    List(Rc<Cons>),
    Vector(Rc<PersistentVector<Self>>),
    Map(Rc<PersistentMap<Self, Self>>),
    Set(Rc<PersistentSet<Self>>),
    Function(Rc<Function>),
    Builtin(Builtin),
}

impl Value {
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "nil",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Symbol(_) => "symbol",
            Self::Keyword(_) => "keyword",
            Self::List(_) => "list",
            Self::Vector(_) => "vector",
            Self::Map(_) => "map",
            Self::Set(_) => "set",
            Self::Function(_) => "function",
            Self::Builtin(_) => "builtin",
        }
    }

    pub const fn is_atom(&self) -> bool {
        !matches!(
            self,
            Self::List(_) | Self::Vector(_) | Self::Map(_) | Self::Set(_)
        )
    }

    pub const fn is_truthy(&self) -> bool {
        !matches!(self, Self::Nil | Self::Bool(false))
    }

    pub const fn as_symbol(&self) -> Option<&InternedSymbol> {
        match self {
            Self::Symbol(s) => Some(s),
            _ => None,
        }
    }
}

pub fn list_from_vec(items: Vec<Value>) -> Value {
    items.into_iter().rev().fold(Value::Nil, |tail, head| {
        Value::List(Rc::new(Cons { head, tail }))
    })
}

pub fn list_to_vec(mut val: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    loop {
        match val {
            Value::List(cons) => {
                out.push(cons.head.clone());
                val = &cons.tail;
            }
            Value::Nil => return out,
            other => {
                out.push(other.clone());
                return out;
            }
        }
    }
}

// --- Display ---

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "()"),
            Self::Bool(true) => write!(f, "true"),
            Self::Bool(false) => write!(f, "false"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) => {
                if n.fract() == 0.0 && n.is_finite() {
                    write!(f, "{n:.1}")
                } else {
                    write!(f, "{n}")
                }
            }
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Symbol(s) => write!(f, "{s}"),
            Self::Keyword(k) => write!(f, ":{k}"),
            Self::List(cons) => {
                write!(f, "(")?;
                write!(f, "{}", cons.head)?;
                let mut current = &cons.tail;
                loop {
                    match current {
                        Self::List(next) => {
                            write!(f, " {}", next.head)?;
                            current = &next.tail;
                        }
                        Self::Nil => break,
                        other => {
                            write!(f, " . {other}")?;
                            break;
                        }
                    }
                }
                write!(f, ")")
            }
            Self::Vector(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Map(entries) => {
                write!(f, "{{")?;
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{key} {value}")?;
                }
                write!(f, "}}")
            }
            Self::Set(items) => {
                write!(f, "#{{")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "}}")
            }
            Self::Function(func) => write!(f, "{func}"),
            Self::Builtin(b) => write!(f, "{b}"),
        }
    }
}

// --- PartialEq (for test assertions) ---

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => *a == *b,
            (Self::Symbol(a), Self::Symbol(b)) | (Self::Keyword(a), Self::Keyword(b)) => a == b,
            (Self::List(a), Self::List(b)) => a.head == b.head && a.tail == b.tail,
            (Self::Vector(a), Self::Vector(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a == b,
            (Self::Set(a), Self::Set(b)) => a == b,
            (Self::Function(a), Self::Function(b)) => Rc::ptr_eq(a, b),
            (Self::Builtin(a), Self::Builtin(b)) => a.name == b.name,
            _ => false,
        }
    }
}

impl Eq for Value {}

// --- Hash ---
//
// Consistent with `PartialEq`: equal values hash equally. Strings and
// collections hash by content; maps and sets combine entry hashes with a
// commutative fold so iteration order does not affect the result.

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Nil => {}
            Self::Bool(b) => b.hash(state),
            Self::Int(n) => n.hash(state),
            Self::Float(n) => n.to_bits().hash(state),
            Self::String(s) => s.hash(state),
            Self::Symbol(s) | Self::Keyword(s) => s.name().hash(state),
            Self::List(cons) => {
                cons.head.hash(state);
                cons.tail.hash(state);
            }
            Self::Vector(items) => {
                for item in items.iter() {
                    item.hash(state);
                }
            }
            Self::Map(entries) => {
                let combined = entries.iter().fold(0u64, |acc, (key, value)| {
                    let mut entry = DefaultHasher::new();
                    key.hash(&mut entry);
                    value.hash(&mut entry);
                    acc.wrapping_add(entry.finish())
                });
                state.write_u64(combined);
            }
            Self::Set(items) => {
                let combined = items.iter().fold(0u64, |acc, item| {
                    let mut entry = DefaultHasher::new();
                    item.hash(&mut entry);
                    acc.wrapping_add(entry.finish())
                });
                state.write_u64(combined);
            }
            Self::Function(func) => std::ptr::hash(Rc::as_ptr(func), state),
            Self::Builtin(b) => b.name.hash(state),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interned_symbols_are_pointer_equal() {
        let a = InternedSymbol::new("foo");
        let b = InternedSymbol::new("foo");
        assert!(a.ptr_eq(&b));
    }

    #[test]
    fn different_symbols_are_not_pointer_equal() {
        let a = InternedSymbol::new("foo");
        let b = InternedSymbol::new("bar");
        assert!(!a.ptr_eq(&b));
    }

    #[test]
    fn list_from_vec_empty() {
        let list = list_from_vec(vec![]);
        assert_eq!(list, Value::Nil);
    }

    #[test]
    fn list_from_vec_builds_cons_chain() {
        let list = list_from_vec(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(list.to_string(), "(1 2 3)");
    }

    #[test]
    fn list_to_vec_roundtrip() {
        let original = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let list = list_from_vec(original.clone());
        let back = list_to_vec(&list);
        assert_eq!(original, back);
    }

    #[test]
    fn nil_is_atomic() {
        assert!(Value::Nil.is_atom());
    }

    #[test]
    fn list_is_not_atomic() {
        let list = list_from_vec(vec![Value::Int(1)]);
        assert!(!list.is_atom());
    }

    #[test]
    fn display_nil() {
        assert_eq!(Value::Nil.to_string(), "()");
    }

    #[test]
    fn display_keyword() {
        let kw = Value::Keyword(InternedSymbol::new("ok"));
        assert_eq!(kw.to_string(), ":ok");
    }

    #[test]
    fn display_string() {
        let s = Value::String(Rc::from("hello"));
        assert_eq!(s.to_string(), "\"hello\"");
    }

    #[test]
    fn display_nested_list() {
        let inner = list_from_vec(vec![Value::Int(1), Value::Int(2)]);
        let outer = list_from_vec(vec![inner, Value::Int(3)]);
        assert_eq!(outer.to_string(), "((1 2) 3)");
    }

    #[test]
    fn truthiness() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Int(0).is_truthy());
        assert!(Value::String(Rc::from("")).is_truthy());
    }

    fn vector(items: Vec<Value>) -> Value {
        Value::Vector(Rc::new(items.into_iter().collect()))
    }

    fn map(entries: Vec<(Value, Value)>) -> Value {
        Value::Map(Rc::new(entries.into_iter().collect()))
    }

    fn set(items: Vec<Value>) -> Value {
        Value::Set(Rc::new(items.into_iter().collect()))
    }

    fn kw(name: &str) -> Value {
        Value::Keyword(InternedSymbol::new(name))
    }

    #[test]
    fn collections_are_not_atoms() {
        assert!(!vector(vec![Value::Int(1)]).is_atom());
        assert!(!map(vec![(kw("a"), Value::Int(1))]).is_atom());
        assert!(!set(vec![kw("a")]).is_atom());
    }

    #[test]
    fn collection_type_names() {
        assert_eq!(vector(vec![]).type_name(), "vector");
        assert_eq!(map(vec![]).type_name(), "map");
        assert_eq!(set(vec![]).type_name(), "set");
    }

    #[test]
    fn display_vector() {
        assert_eq!(
            vector(vec![Value::Int(1), Value::Int(2), Value::Int(3)]).to_string(),
            "[1 2 3]"
        );
        assert_eq!(vector(vec![]).to_string(), "[]");
    }

    #[test]
    fn display_single_entry_map() {
        assert_eq!(map(vec![(kw("a"), Value::Int(1))]).to_string(), "{:a 1}");
    }

    #[test]
    fn display_single_element_set() {
        assert_eq!(set(vec![kw("x")]).to_string(), "#{:x}");
    }

    #[test]
    fn structural_equality_independent_of_insertion_order() {
        let a = map(vec![(kw("a"), Value::Int(1)), (kw("b"), Value::Int(2))]);
        let b = map(vec![(kw("b"), Value::Int(2)), (kw("a"), Value::Int(1))]);
        assert_eq!(a, b);

        let s1 = set(vec![kw("a"), kw("b"), kw("c")]);
        let s2 = set(vec![kw("c"), kw("a"), kw("b")]);
        assert_eq!(s1, s2);

        assert_eq!(
            vector(vec![Value::Int(1), Value::Int(2)]),
            vector(vec![Value::Int(1), Value::Int(2)])
        );
        assert_ne!(
            vector(vec![Value::Int(1), Value::Int(2)]),
            vector(vec![Value::Int(2), Value::Int(1)])
        );
    }

    #[test]
    fn equal_collections_hash_equally() {
        fn hash(value: &Value) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }
        let a = map(vec![(kw("a"), Value::Int(1)), (kw("b"), Value::Int(2))]);
        let b = map(vec![(kw("b"), Value::Int(2)), (kw("a"), Value::Int(1))]);
        assert_eq!(hash(&a), hash(&b));

        // Collections are usable as map keys / set members.
        let nested = set(vec![
            vector(vec![Value::Int(1)]),
            vector(vec![Value::Int(2)]),
        ]);
        assert_eq!(
            nested,
            set(vec![
                vector(vec![Value::Int(2)]),
                vector(vec![Value::Int(1)])
            ])
        );
    }
}

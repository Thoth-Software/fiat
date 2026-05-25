use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::error::Error;
use crate::value::{InternedSymbol, Value};

#[derive(Debug)]
pub struct Env {
    bindings: RefCell<HashMap<InternedSymbol, Value>>,
    capabilities: RefCell<HashSet<String>>,
    parent: Option<Rc<Self>>,
}

impl Env {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            bindings: RefCell::new(HashMap::new()),
            capabilities: RefCell::new(HashSet::new()),
            parent: None,
        })
    }

    pub fn with_parent(parent: Rc<Self>) -> Rc<Self> {
        Rc::new(Self {
            bindings: RefCell::new(HashMap::new()),
            capabilities: RefCell::new(HashSet::new()),
            parent: Some(parent),
        })
    }

    pub fn get(&self, name: &InternedSymbol) -> Result<Value, Error> {
        if let Some(val) = self.bindings.borrow().get(name) {
            return Ok(val.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.get(name);
        }
        Err(Error::unbound_symbol(name.name()))
    }

    pub fn set(&self, name: InternedSymbol, value: Value) {
        self.bindings.borrow_mut().insert(name, value);
    }

    pub fn register_capability(&self, name: String) {
        self.capabilities.borrow_mut().insert(name);
    }

    pub fn has_capability(&self, name: &str) -> bool {
        if self.capabilities.borrow().contains(name) {
            return true;
        }
        if let Some(parent) = &self.parent {
            return parent.has_capability(name);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let env = Env::new();
        let sym = InternedSymbol::new("x");
        env.set(sym.clone(), Value::Int(42));
        assert_eq!(env.get(&sym).ok(), Some(Value::Int(42)));
    }

    #[test]
    fn parent_lookup() {
        let parent = Env::new();
        let sym = InternedSymbol::new("x");
        parent.set(sym.clone(), Value::Int(10));

        let child = Env::with_parent(parent);
        assert_eq!(child.get(&sym).ok(), Some(Value::Int(10)));
    }

    #[test]
    fn child_shadows_parent() {
        let parent = Env::new();
        let sym = InternedSymbol::new("x");
        parent.set(sym.clone(), Value::Int(10));

        let child = Env::with_parent(parent);
        child.set(sym.clone(), Value::Int(20));
        assert_eq!(child.get(&sym).ok(), Some(Value::Int(20)));
    }

    #[test]
    fn unbound_symbol_error() {
        let env = Env::new();
        let sym = InternedSymbol::new("nope");
        let result = env.get(&sym);
        assert!(result.is_err());
    }
}

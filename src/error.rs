use std::fmt;

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    TypeError,
    UnboundSymbol,
    NotCallable,
    MalformedSpecialForm,
    ArityError,
    DivisionByZero,
    RuntimeError,
}

impl Error {
    pub fn type_error(expected: &str, got: &str) -> Self {
        Self {
            kind: ErrorKind::TypeError,
            message: format!("expected {expected}, got {got}"),
        }
    }

    pub fn unbound_symbol(name: &str) -> Self {
        Self {
            kind: ErrorKind::UnboundSymbol,
            message: format!("unbound symbol: {name}"),
        }
    }

    pub fn not_callable(desc: &str) -> Self {
        Self {
            kind: ErrorKind::NotCallable,
            message: format!("not callable: {desc}"),
        }
    }

    pub fn malformed(form: &str, detail: &str) -> Self {
        Self {
            kind: ErrorKind::MalformedSpecialForm,
            message: format!("malformed {form}: {detail}"),
        }
    }

    pub fn arity(name: &str, expected: usize, got: usize) -> Self {
        Self {
            kind: ErrorKind::ArityError,
            message: format!("{name} expects {expected} argument(s), got {got}"),
        }
    }

    pub fn first_on_empty_list() -> Self {
        Self {
            kind: ErrorKind::RuntimeError,
            message: "first called on empty list".to_string(),
        }
    }

    pub fn division_by_zero() -> Self {
        Self {
            kind: ErrorKind::DivisionByZero,
            message: "division by zero".to_string(),
        }
    }

    pub fn is_on_collection(type_name: &str) -> Self {
        Self {
            kind: ErrorKind::TypeError,
            message: format!("is? called on collection: {type_name}"),
        }
    }

    pub fn bind_non_list(type_name: &str) -> Self {
        Self {
            kind: ErrorKind::TypeError,
            message: format!("bind requires a list as second argument, got {type_name}"),
        }
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::RuntimeError,
            message: msg.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {}

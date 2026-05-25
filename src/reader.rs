use std::rc::Rc;

use im_rc::{HashMap as PersistentMap, HashSet as PersistentSet, Vector as PersistentVector};

use crate::error::Error;
use crate::value::{InternedSymbol, Value, list_from_vec};

pub fn read(source: &str) -> Result<Vec<Value>, Error> {
    let tokens = tokenize(source)?;
    let mut pos = 0;
    let mut forms = Vec::new();
    while pos < tokens.len() {
        let (val, next) = read_form(&tokens, pos)?;
        forms.push(val);
        pos = next;
    }
    Ok(forms)
}

pub fn read_one(source: &str) -> Result<Value, Error> {
    let tokens = tokenize(source)?;
    if tokens.is_empty() {
        return Err(Error::runtime("empty input"));
    }
    let (val, _) = read_form(&tokens, 0)?;
    Ok(val)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    HashBrace,
    Quote,
    Symbol(String),
    Keyword(String),
    StringLit(String),
    Int(i64),
    Float(f64),
}

fn tokenize(source: &str) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' | ',' => i += 1,

            ';' if i + 1 < chars.len() && chars[i + 1] == ';' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }

            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '[' => {
                tokens.push(Token::LBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
                i += 1;
            }
            '{' => {
                tokens.push(Token::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Token::RBrace);
                i += 1;
            }
            '#' if i + 1 < chars.len() && chars[i + 1] == '{' => {
                tokens.push(Token::HashBrace);
                i += 2;
            }
            '\'' => {
                tokens.push(Token::Quote);
                i += 1;
            }

            '"' => {
                let (s, next) = read_string_literal(&chars, i + 1)?;
                tokens.push(Token::StringLit(s));
                i = next;
            }

            ':' => {
                i += 1;
                let start = i;
                while i < chars.len() && is_symbol_char(chars[i]) {
                    i += 1;
                }
                if i == start {
                    return Err(Error::runtime("expected keyword name after ':'"));
                }
                let name: String = chars[start..i].iter().collect();
                tokens.push(Token::Keyword(name));
            }

            c if is_symbol_start(c) || c == '-' || c == '+' => {
                let start = i;
                i += 1;
                while i < chars.len() && is_symbol_char(chars[i]) {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                if let Some(tok) = try_parse_number(&text) {
                    tokens.push(tok);
                } else {
                    tokens.push(Token::Symbol(text));
                }
            }

            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && is_symbol_char(chars[i]) {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                if let Some(tok) = try_parse_number(&text) {
                    tokens.push(tok);
                } else {
                    return Err(Error::runtime(format!("invalid number: {text}")));
                }
            }

            c => return Err(Error::runtime(format!("unexpected character: {c}"))),
        }
    }

    Ok(tokens)
}

fn read_string_literal(chars: &[char], start: usize) -> Result<(String, usize), Error> {
    let mut i = start;
    let mut s = String::new();
    while i < chars.len() && chars[i] != '"' {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 1;
            match chars[i] {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                '\\' => s.push('\\'),
                '"' => s.push('"'),
                c => {
                    s.push('\\');
                    s.push(c);
                }
            }
        } else {
            s.push(chars[i]);
        }
        i += 1;
    }
    if i >= chars.len() {
        return Err(Error::runtime("unterminated string"));
    }
    Ok((s, i + 1))
}

fn is_symbol_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '_' | '!' | '?' | '*' | '/' | '<' | '>' | '=' | '%')
}

fn is_symbol_char(c: char) -> bool {
    is_symbol_start(c) || c == '-' || c == '+' || c == '.' || c.is_ascii_digit()
}

fn try_parse_number(text: &str) -> Option<Token> {
    if let Ok(n) = text.parse::<i64>() {
        return Some(Token::Int(n));
    }
    if text.contains('.')
        && let Ok(n) = text.parse::<f64>()
    {
        return Some(Token::Float(n));
    }
    None
}

fn read_form(tokens: &[Token], pos: usize) -> Result<(Value, usize), Error> {
    if pos >= tokens.len() {
        return Err(Error::runtime("unexpected end of input"));
    }

    match &tokens[pos] {
        Token::LParen => read_list(tokens, pos + 1),
        Token::RParen => Err(Error::runtime("unexpected ')'")),
        Token::LBracket => read_vector(tokens, pos + 1),
        Token::RBracket => Err(Error::runtime("unexpected ']'")),
        Token::LBrace => read_map(tokens, pos + 1),
        Token::RBrace => Err(Error::runtime("unexpected '}'")),
        Token::HashBrace => read_set(tokens, pos + 1),
        Token::Quote => {
            let (val, next) = read_form(tokens, pos + 1)?;
            let behold = Value::Symbol(InternedSymbol::new("behold"));
            let quoted = list_from_vec(vec![behold, val]);
            Ok((quoted, next))
        }
        Token::Int(n) => Ok((Value::Int(*n), pos + 1)),
        Token::Float(n) => Ok((Value::Float(*n), pos + 1)),
        Token::StringLit(s) => Ok((Value::String(Rc::from(s.as_str())), pos + 1)),
        Token::Keyword(k) => Ok((Value::Keyword(InternedSymbol::new(k)), pos + 1)),
        Token::Symbol(s) => {
            let val = match s.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "nil" => Value::Nil,
                _ => Value::Symbol(InternedSymbol::new(s)),
            };
            Ok((val, pos + 1))
        }
    }
}

fn read_list(tokens: &[Token], mut pos: usize) -> Result<(Value, usize), Error> {
    let mut items = Vec::new();
    loop {
        if pos >= tokens.len() {
            return Err(Error::runtime("unterminated list, expected ')'"));
        }
        if tokens[pos] == Token::RParen {
            return Ok((list_from_vec(items), pos + 1));
        }
        let (val, next) = read_form(tokens, pos)?;
        items.push(val);
        pos = next;
    }
}

fn read_vector(tokens: &[Token], mut pos: usize) -> Result<(Value, usize), Error> {
    let mut items = PersistentVector::new();
    loop {
        if pos >= tokens.len() {
            return Err(Error::runtime("unterminated vector, expected ']'"));
        }
        if tokens[pos] == Token::RBracket {
            return Ok((Value::Vector(Rc::new(items)), pos + 1));
        }
        let (val, next) = read_form(tokens, pos)?;
        items.push_back(val);
        pos = next;
    }
}

fn read_map(tokens: &[Token], mut pos: usize) -> Result<(Value, usize), Error> {
    let mut entries = PersistentMap::new();
    loop {
        if pos >= tokens.len() {
            return Err(Error::runtime("unterminated map, expected '}'"));
        }
        if tokens[pos] == Token::RBrace {
            return Ok((Value::Map(Rc::new(entries)), pos + 1));
        }
        let (key, next) = read_form(tokens, pos)?;
        pos = next;
        if pos >= tokens.len() || tokens[pos] == Token::RBrace {
            return Err(Error::runtime(
                "map literal requires an even number of forms",
            ));
        }
        let (value, next) = read_form(tokens, pos)?;
        entries.insert(key, value);
        pos = next;
    }
}

fn read_set(tokens: &[Token], mut pos: usize) -> Result<(Value, usize), Error> {
    let mut items = PersistentSet::new();
    loop {
        if pos >= tokens.len() {
            return Err(Error::runtime("unterminated set, expected '}'"));
        }
        if tokens[pos] == Token::RBrace {
            return Ok((Value::Set(Rc::new(items)), pos + 1));
        }
        let (val, next) = read_form(tokens, pos)?;
        items.insert(val);
        pos = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_str(s: &str) -> Value {
        read_one(s).unwrap_or_else(|e| panic!("read error: {e}"))
    }

    #[test]
    fn read_integer() {
        assert_eq!(read_str("42"), Value::Int(42));
    }

    #[test]
    fn read_negative_integer() {
        assert_eq!(read_str("-7"), Value::Int(-7));
    }

    #[test]
    fn read_float() {
        assert_eq!(read_str("3.14"), Value::Float(3.14));
    }

    #[test]
    fn read_string() {
        assert_eq!(read_str("\"hello\""), Value::String(Rc::from("hello")));
    }

    #[test]
    fn read_bool_and_nil() {
        assert_eq!(read_str("true"), Value::Bool(true));
        assert_eq!(read_str("false"), Value::Bool(false));
        assert_eq!(read_str("nil"), Value::Nil);
    }

    #[test]
    fn read_symbol() {
        assert_eq!(read_str("foo"), Value::Symbol(InternedSymbol::new("foo")));
    }

    #[test]
    fn read_keyword() {
        assert_eq!(read_str(":ok"), Value::Keyword(InternedSymbol::new("ok")));
    }

    #[test]
    fn read_empty_list() {
        assert_eq!(read_str("()"), Value::Nil);
    }

    #[test]
    fn read_list() {
        let val = read_str("(+ 1 2)");
        assert_eq!(val.to_string(), "(+ 1 2)");
    }

    #[test]
    fn read_nested_list() {
        let val = read_str("(a (b c) d)");
        assert_eq!(val.to_string(), "(a (b c) d)");
    }

    #[test]
    fn read_quote_shorthand() {
        let val = read_str("'x");
        assert_eq!(val.to_string(), "(behold x)");
    }

    #[test]
    fn read_quote_list() {
        let val = read_str("'(1 2 3)");
        assert_eq!(val.to_string(), "(behold (1 2 3))");
    }

    #[test]
    fn read_with_comments() {
        let vals = read(";; this is a comment\n42").unwrap();
        assert_eq!(vals.len(), 1);
        assert_eq!(vals[0], Value::Int(42));
    }

    #[test]
    fn read_multiple_forms() {
        let vals = read("1 2 3").unwrap();
        assert_eq!(vals.len(), 3);
    }

    #[test]
    fn unterminated_list_error() {
        assert!(read_one("(a b").is_err());
    }

    #[test]
    fn unterminated_string_error() {
        assert!(read_one("\"hello").is_err());
    }

    // --- Vector literals ---

    #[test]
    fn read_empty_vector() {
        assert_eq!(read_str("[]").to_string(), "[]");
    }

    #[test]
    fn read_vector_of_ints() {
        assert_eq!(read_str("[1 2 3]").to_string(), "[1 2 3]");
    }

    #[test]
    fn read_nested_vector() {
        assert_eq!(read_str("[[1 2] [3 4]]").to_string(), "[[1 2] [3 4]]");
    }

    #[test]
    fn unterminated_vector_error() {
        assert!(read_one("[1 2").is_err());
    }

    #[test]
    fn unexpected_rbracket_error() {
        assert!(read_one("]").is_err());
    }

    // --- Map literals ---

    #[test]
    fn read_empty_map() {
        assert_eq!(read_str("{}").to_string(), "{}");
    }

    #[test]
    fn read_single_entry_map() {
        assert_eq!(read_str("{:a 1}").to_string(), "{:a 1}");
    }

    #[test]
    fn odd_map_elements_error() {
        assert!(read_one("{:a 1 :b}").is_err());
    }

    #[test]
    fn unterminated_map_error() {
        assert!(read_one("{:a 1").is_err());
    }

    #[test]
    fn unexpected_rbrace_error() {
        assert!(read_one("}").is_err());
    }

    // --- Set literals ---

    #[test]
    fn read_empty_set() {
        assert_eq!(read_str("#{}").to_string(), "#{}");
    }

    #[test]
    fn read_single_element_set() {
        assert_eq!(read_str("#{:x}").to_string(), "#{:x}");
    }

    #[test]
    fn unterminated_set_error() {
        assert!(read_one("#{1 2").is_err());
    }

    // --- Nesting across collection types ---

    #[test]
    fn vector_of_maps() {
        assert_eq!(read_str("[{:a 1} {:b 2}]").to_string(), "[{:a 1} {:b 2}]");
    }

    #[test]
    fn map_with_vector_value() {
        assert_eq!(read_str("{:v [1 2 3]}").to_string(), "{:v [1 2 3]}");
    }

    #[test]
    fn list_containing_vector_and_set() {
        let val = read_str("(foo [1 2] #{:a :b})");
        let s = val.to_string();
        assert!(s.starts_with("(foo [1 2] #{"));
        assert!(s.contains(":a"));
        assert!(s.contains(":b"));
    }

    // --- Round-trip tests ---

    #[test]
    fn roundtrip_vector() {
        let input = "[1 2 3]";
        assert_eq!(read_str(input).to_string(), input);
    }

    #[test]
    fn roundtrip_single_entry_map() {
        let input = "{:a 1}";
        assert_eq!(read_str(input).to_string(), input);
    }

    #[test]
    fn roundtrip_set_normalized() {
        let val = read_str("#{1 2 3}");
        let s = val.to_string();
        assert!(s.starts_with("#{"));
        assert!(s.ends_with('}'));
        // Parse back and compare structurally
        let val2 = read_str(&s);
        assert_eq!(val, val2);
    }
}

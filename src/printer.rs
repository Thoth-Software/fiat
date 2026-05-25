use crate::value::Value;

pub fn pr_str(val: &Value) -> String {
    val.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::{read, read_one};

    fn roundtrip(source: &str) -> String {
        let val = read_one(source).unwrap_or_else(|e| panic!("read error on {source:?}: {e}"));
        pr_str(&val)
    }

    #[test]
    fn roundtrip_integer() {
        assert_eq!(roundtrip("42"), "42");
    }

    #[test]
    fn roundtrip_negative_integer() {
        assert_eq!(roundtrip("-7"), "-7");
    }

    #[test]
    fn roundtrip_float() {
        assert_eq!(roundtrip("3.14"), "3.14");
    }

    #[test]
    fn roundtrip_whole_float() {
        assert_eq!(roundtrip("2.0"), "2.0");
    }

    #[test]
    fn roundtrip_string() {
        assert_eq!(roundtrip("\"hello world\""), "\"hello world\"");
    }

    #[test]
    fn roundtrip_symbol() {
        assert_eq!(roundtrip("foo"), "foo");
    }

    #[test]
    fn roundtrip_keyword() {
        assert_eq!(roundtrip(":err"), ":err");
    }

    #[test]
    fn roundtrip_booleans() {
        assert_eq!(roundtrip("true"), "true");
        assert_eq!(roundtrip("false"), "false");
    }

    #[test]
    fn roundtrip_nil() {
        assert_eq!(roundtrip("nil"), "()");
    }

    #[test]
    fn roundtrip_empty_list() {
        assert_eq!(roundtrip("()"), "()");
    }

    #[test]
    fn roundtrip_simple_list() {
        assert_eq!(roundtrip("(+ 1 2)"), "(+ 1 2)");
    }

    #[test]
    fn roundtrip_nested_list() {
        assert_eq!(roundtrip("(a (b c) d)"), "(a (b c) d)");
    }

    #[test]
    fn roundtrip_keywords_in_list() {
        assert_eq!(roundtrip("(:ok :err :player)"), "(:ok :err :player)");
    }

    #[test]
    fn roundtrip_quote() {
        assert_eq!(roundtrip("'x"), "(behold x)");
    }

    #[test]
    fn roundtrip_mixed() {
        assert_eq!(
            roundtrip("(fiat add (a b) (+ a b))"),
            "(fiat add (a b) (+ a b))"
        );
    }

    #[test]
    fn roundtrip_multiline_program() {
        let source = "(fiat double (x) (* x 2))\n(double 21)";
        let forms = read(source).unwrap();
        let printed: Vec<String> = forms.iter().map(|v| pr_str(v)).collect();
        assert_eq!(printed[0], "(fiat double (x) (* x 2))");
        assert_eq!(printed[1], "(double 21)");
    }
}

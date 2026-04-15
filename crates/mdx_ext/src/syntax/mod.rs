//! Shared tokenizer helpers for the custom syntax subparsers.

use serde_json::Value;

/// Parse a sequence of `key=value` pairs from a string.
///
/// Supports:
/// * double-quoted strings with `\"` and `\\` escapes
/// * integers and floats (stored as JSON numbers)
/// * `true` / `false` / `null`
/// * bare identifiers as strings
///
/// Invalid runs are silently skipped — callers may wrap this helper with
/// their own diagnostic emission.
pub fn parse_inline_attrs(input: &str) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        // key
        let key_start = i;
        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' {
                i += 1;
            } else {
                break;
            }
        }
        if i == key_start {
            // garbage; skip one byte and retry
            i += 1;
            continue;
        }
        let key = input[key_start..i].to_string();
        // optional `=`
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            // bare flag → true
            out.push((key, Value::Bool(true)));
            continue;
        }
        i += 1; // consume '='
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            out.push((key, Value::Null));
            break;
        }
        // value
        let (value, consumed) = parse_value(&input[i..]);
        i += consumed;
        out.push((key, value));
    }
    out
}

fn parse_value(s: &str) -> (Value, usize) {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return (Value::Null, 0);
    }
    match bytes[0] {
        b'"' => parse_quoted(s),
        b'\'' => parse_quoted(s),
        _ => parse_bare(s),
    }
}

fn parse_quoted(s: &str) -> (Value, usize) {
    let quote = s.as_bytes()[0];
    let mut out = String::new();
    let mut i = 1usize;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            match next {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                b'\'' => out.push('\''),
                other => {
                    out.push('\\');
                    out.push(other as char);
                }
            }
            i += 2;
        } else if c == quote {
            return (Value::String(out), i + 1);
        } else {
            // UTF-8-safe push: find next char boundary
            let ch_start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
                i += 1;
            }
            out.push_str(&s[ch_start..i]);
        }
    }
    // unterminated → take whatever we have
    (Value::String(out), bytes.len())
}

fn parse_bare(s: &str) -> (Value, usize) {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() || c == b'}' {
            break;
        }
        i += 1;
    }
    let token = &s[..i];
    let v = match token {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            if let Ok(n) = token.parse::<i64>() {
                Value::Number(n.into())
            } else if let Ok(f) = token.parse::<f64>() {
                serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or(Value::String(token.to_string()))
            } else {
                Value::String(token.to_string())
            }
        }
    };
    (v, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_attrs() {
        let v = parse_inline_attrs(r#"name="foo bar" count=3 flag true_value=true"#);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].0, "name");
        assert_eq!(v[0].1, Value::String("foo bar".into()));
        assert_eq!(v[1].0, "count");
        assert_eq!(v[1].1, Value::Number(3.into()));
        assert_eq!(v[2].1, Value::Bool(true));
        assert_eq!(v[3].1, Value::Bool(true));
    }
}

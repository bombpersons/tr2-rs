//! Saving a changed document without reformatting what did not change.
//!
//! Every subtree equal to the original keeps its original bytes. A changed
//! array or object reuses the original's brackets, separators and key
//! spelling, so an edited room in a compact file stays compact and every
//! other room is byte-for-byte the same. Values with no original counterpart
//! are formatted after a sibling where there is one, otherwise compactly.
use serde_json::Value;

/// An array or object member: its key's span (objects), and its value's start and end.
type Member = (Option<(usize, usize)>, usize, usize);

/// `new` written in the style of `original`, whose text is `bytes`.
pub fn splice(new: &Value, original: &Value, bytes: &[u8]) -> Vec<u8> {
    let start = skip_ws(bytes, 0);
    let end = value_end(bytes, start);
    let mut out = Vec::with_capacity(bytes.len() + 1024);
    out.extend_from_slice(&bytes[..start]);
    emit(&mut out, new, Some((original, &bytes[start..end])));
    out.extend_from_slice(&bytes[end..]);
    out
}

/// Write `value`, formatted like `template` (an equal or similar value and its text).
fn emit(out: &mut Vec<u8>, value: &Value, template: Option<(&Value, &[u8])>) {
    if let Some((old, text)) = template {
        if old == value {
            out.extend_from_slice(text);
            return;
        }
        match (value, old) {
            (Value::Array(items), Value::Array(old_items)) => return array(out, items, old_items, text),
            (Value::Object(map), Value::Object(old_map)) => return object(out, map, old_map, text),
            _ => (),
        }
    }
    out.extend_from_slice(serde_json::to_string(value).expect("JSON values serialize").as_bytes());
}

fn array(out: &mut Vec<u8>, items: &[Value], old: &[Value], text: &[u8]) {
    let parts = members(text);
    let (open, separator, close) = layout(text, &parts);
    let separator = separator_text(separator, parts.len());
    out.extend_from_slice(open);
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(&separator);
        }
        // Appended entries look like the last original one.
        let j = i.min(old.len().saturating_sub(1));
        emit(out, item, parts.get(j).map(|&(_, start, end)| (&old[j], &text[start..end])));
    }
    out.extend_from_slice(close);
}

fn object(out: &mut Vec<u8>, map: &serde_json::Map<String, Value>, old: &serde_json::Map<String, Value>, text: &[u8]) {
    let parts = members(text);
    let (open, separator, close) = layout(text, &parts);
    let separator = separator_text(separator, parts.len());
    // Original order first, then added keys.
    let mut keys: Vec<(&str, Option<usize>)> = vec![];
    for (i, &(key, _, _)) in parts.iter().enumerate() {
        let name = key.and_then(|k| serde_json::from_slice::<String>(&text[k.0..k.1]).ok()).unwrap_or_default();
        if let Some((name, _)) = map.get_key_value(&name) {
            keys.push((name.as_str(), Some(i)));
        }
    }
    for name in map.keys() {
        if !old.contains_key(name) {
            keys.push((name.as_str(), None));
        }
    }
    let colon = parts.first().and_then(|&(key, start, _)| key.map(|k| &text[k.1..start])).unwrap_or(b":");
    out.extend_from_slice(open);
    for (n, &(name, index)) in keys.iter().enumerate() {
        if n > 0 {
            out.extend_from_slice(&separator);
        }
        match index {
            Some(i) => {
                let (key, start, end) = parts[i];
                let key = key.expect("object members have keys");
                out.extend_from_slice(&text[key.0..start]);
                emit(out, &map[name], Some((&old[name], &text[start..end])));
            }
            None => {
                out.extend_from_slice(serde_json::to_string(name).expect("strings serialize").as_bytes());
                out.extend_from_slice(colon);
                emit(out, &map[name], None);
            }
        }
    }
    out.extend_from_slice(close);
}

/// With one member, `layout` gives the opening text: a comma and its whitespace.
fn separator_text(text: &[u8], members: usize) -> Vec<u8> {
    if members == 1 { [b",".as_slice(), &text[1..]].concat() } else { text.to_vec() }
}

/// The text before the first member, between members, and after the last.
fn layout<'a>(text: &'a [u8], parts: &[Member]) -> (&'a [u8], &'a [u8], &'a [u8]) {
    let first = |p: &Member| p.0.map_or(p.1, |k| k.0);
    match parts {
        [] => (&text[..1], b",", &text[text.len() - 1..]),
        // One member: separate added ones like the opening bracket does.
        [only] => (&text[..first(only)], &text[..first(only)], &text[only.2..]),
        [a, b, ..] => (&text[..first(a)], &text[a.2..first(b)], &text[parts.last().unwrap().2..]),
    }
}

/// Each member of the array or object `text`: its key's span (objects), and
/// its value's start and end.
fn members(text: &[u8]) -> Vec<Member> {
    let object = text[0] == b'{';
    let mut parts = vec![];
    let mut i = skip_ws(text, 1);
    while i < text.len() - 1 && text[i] != b']' && text[i] != b'}' {
        let key = object.then(|| {
            let end = value_end(text, i);
            let key = (i, end);
            i = skip_ws(text, end) + 1;
            i = skip_ws(text, i);
            key
        });
        let end = value_end(text, i);
        parts.push((key, i, end));
        i = skip_ws(text, end);
        if text[i] == b',' {
            i = skip_ws(text, i + 1);
        }
    }
    parts
}

fn skip_ws(text: &[u8], mut i: usize) -> usize {
    while i < text.len() && text[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The end of the JSON value starting at `i`.
fn value_end(text: &[u8], mut i: usize) -> usize {
    match text[i] {
        b'"' => {
            i += 1;
            while text[i] != b'"' {
                i += if text[i] == b'\\' { 2 } else { 1 };
            }
            i + 1
        }
        b'[' | b'{' => {
            let mut depth = 0;
            loop {
                match text[i] {
                    b'"' => {
                        i = value_end(text, i);
                        continue;
                    }
                    b'[' | b'{' => depth += 1,
                    b']' | b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return i + 1;
                        }
                    }
                    _ => (),
                }
                i += 1;
            }
        }
        _ => {
            while i < text.len() && !matches!(text[i], b',' | b']' | b'}') && !text[i].is_ascii_whitespace() {
                i += 1;
            }
            i
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn check(text: &str, new: Value) -> String {
        let old: Value = serde_json::from_str(text).unwrap();
        let out = String::from_utf8(splice(&new, &old, text.as_bytes())).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), new, "{out}");
        out
    }

    #[test]
    fn untouched_subtrees_keep_their_bytes_and_changed_ones_keep_the_style() {
        let text = "\n{ \"z\" : [1,  2, {\"a\":\"x\\\"]\"}],\n  \"a\": {\"k\": [ 3 ]} }\n";
        assert_eq!(check(text, json!({"z": [1, 2, {"a": "x\"]"}], "a": {"k": [3]}})), text);
        assert_eq!(
            check(text, json!({"z": [1, 2, {"a": "x\"]"}], "a": {"k": [4, 5]}})),
            "\n{ \"z\" : [1,  2, {\"a\":\"x\\\"]\"}],\n  \"a\": {\"k\": [ 4, 5 ]} }\n"
        );
        // Key order is the file's; added keys follow, and removed ones go.
        assert_eq!(
            check(text, json!({"a": {}, "b": null, "z": []})),
            "\n{ \"z\" : [],\n  \"a\": {},\n  \"b\" : null }\n"
        );
        let pretty = "[\n  [1, 2],\n  [3, 4]\n]";
        assert_eq!(check(pretty, json!([[1, 2], [3, 5], [6, 7]])), "[\n  [1, 2],\n  [3, 5],\n  [6, 7]\n]");
        assert_eq!(check("[]", json!([1, [2]])), "[1,[2]]");
        assert_eq!(check("{\"a\":1}", json!({"a": 1, "b": 2})), "{\"a\":1,\"b\":2}");
    }
}

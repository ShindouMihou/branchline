const UNSUPPORTED: &str =
    "This shell structure cannot be separated safely; review the whole command.";

fn quoted(bytes: &[u8], mut index: usize, quote: u8, depth: usize) -> Result<usize, String> {
    while index < bytes.len() {
        match bytes[index] {
            value if value == quote => return Ok(index + 1),
            b'\\' if quote == b'"' => index += 2,
            b'$' if quote == b'"' => index = expansion(bytes, index, depth)?,
            b'`' if quote == b'"' => return Err(UNSUPPORTED.into()),
            _ => index += 1,
        }
    }
    Err("The shell text has an unfinished quote.".into())
}

fn expansion(bytes: &[u8], index: usize, depth: usize) -> Result<usize, String> {
    match bytes.get(index + 1) {
        Some(b'(') => balanced(bytes, index + 2, b')', depth + 1),
        Some(b'{') => balanced(bytes, index + 2, b'}', depth + 1),
        Some(b'\'' | b'"') => Err(UNSUPPORTED.into()),
        _ => Ok(index + 1),
    }
}

fn balanced(bytes: &[u8], mut index: usize, end: u8, depth: usize) -> Result<usize, String> {
    if depth > 32 {
        return Err("Shell nesting exceeds the static preview limit.".into());
    }
    while index < bytes.len() {
        match bytes[index] {
            value if value == end => return Ok(index + 1),
            b'\'' | b'"' => index = quoted(bytes, index + 1, bytes[index], depth)?,
            b'\\' => index += 2,
            b'$' => index = expansion(bytes, index, depth)?,
            b'(' => index = balanced(bytes, index + 1, b')', depth + 1)?,
            b'{' => index = balanced(bytes, index + 1, b'}', depth + 1)?,
            b'`' | b'<' | b'>' => return Err(UNSUPPORTED.into()),
            b'#' if index == 0
                || bytes[index - 1].is_ascii_whitespace()
                || b";(|&".contains(&bytes[index - 1]) =>
            {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    Err("The shell text has an unfinished expansion.".into())
}

pub(super) fn split(script: &str) -> Result<Vec<&str>, String> {
    let bytes = script.as_bytes();
    let (mut index, mut start) = (0, 0);
    let mut clauses = Vec::new();
    while index < bytes.len() {
        let ch = bytes[index];
        match ch {
            b'\'' | b'"' => index = quoted(bytes, index + 1, ch, 0)?,
            b'\\' => {
                if index + 1 == bytes.len() {
                    return Err(UNSUPPORTED.into());
                }
                index += 2;
            }
            b'$' => index = expansion(bytes, index, 0)?,
            b'`' | b'<' | b'>' | b'|' | b'(' | b')' | b'{' | b'}' => return Err(UNSUPPORTED.into()),
            b'#' if index == 0
                || bytes[index - 1].is_ascii_whitespace()
                || b";&".contains(&bytes[index - 1]) =>
            {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'&' | b';' | b'\n' => {
                if ch == b'&' && bytes.get(index + 1) != Some(&b'&')
                    || ch == b';' && bytes.get(index + 1) == Some(&b';')
                {
                    return Err(UNSUPPORTED.into());
                }
                clauses.push(&script[start..index]);
                index += if ch == b'&' { 2 } else { 1 };
                start = index;
            }
            _ => index += 1,
        }
    }
    clauses.push(&script[start..]);
    for clause in &clauses {
        if clause.split_whitespace().next().is_some_and(|word| {
            matches!(
                word,
                "if" | "then"
                    | "else"
                    | "elif"
                    | "fi"
                    | "for"
                    | "while"
                    | "until"
                    | "do"
                    | "done"
                    | "case"
                    | "esac"
                    | "in"
                    | "function"
                    | "select"
                    | "!"
            )
        }) {
            return Err(UNSUPPORTED.into());
        }
    }
    Ok(clauses)
}

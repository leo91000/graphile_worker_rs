mod keywords;

pub fn escape_identifier(identifier: &str) -> String {
    if is_safe_identifier(identifier) {
        return identifier.to_string();
    }

    let quotes = identifier.bytes().filter(|byte| *byte == b'"').count();
    let mut result = String::with_capacity(identifier.len() + quotes + 2);
    result.push('"');

    for ch in identifier.chars() {
        if ch == '"' {
            result.push('"');
        }
        result.push(ch);
    }

    result.push('"');
    result
}

fn is_safe_identifier(identifier: &str) -> bool {
    let Some(first) = identifier.bytes().next() else {
        return false;
    };

    if !matches!(first, b'a'..=b'z' | b'_') {
        return false;
    }

    if !identifier
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_'))
    {
        return false;
    }

    !keywords::requires_quotes(identifier)
}

#[cfg(test)]
mod tests;

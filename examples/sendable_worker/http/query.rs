use std::collections::HashMap;

pub(super) fn parse_query_params(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|param| {
            let mut parts = param.split('=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => {
                    Some((simple_url_decode(key), simple_url_decode(value)))
                }
                _ => None,
            }
        })
        .collect()
}

fn simple_url_decode(s: &str) -> String {
    s.replace("%20", " ")
        .replace("%21", "!")
        .replace("%22", "\"")
        .replace("%23", "#")
        .replace("%24", "$")
        .replace("%25", "%")
        .replace("%26", "&")
        .replace("%27", "'")
        .replace("%28", "(")
        .replace("%29", ")")
        .replace("%2A", "*")
        .replace("%2B", "+")
        .replace("%2C", ",")
        .replace("%2F", "/")
        .replace("%3A", ":")
        .replace("%3B", ";")
        .replace("%3D", "=")
        .replace("%3F", "?")
        .replace("%40", "@")
        .replace("%5B", "[")
        .replace("%5D", "]")
}

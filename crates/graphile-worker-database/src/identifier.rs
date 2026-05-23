const KEYWORDS_REQUIRING_QUOTES: &[&str] = &[
    "all",
    "analyse",
    "analyze",
    "and",
    "any",
    "array",
    "as",
    "asc",
    "asymmetric",
    "authorization",
    "between",
    "bigint",
    "binary",
    "bit",
    "boolean",
    "both",
    "case",
    "cast",
    "char",
    "character",
    "check",
    "coalesce",
    "collate",
    "collation",
    "column",
    "concurrently",
    "constraint",
    "create",
    "cross",
    "current_catalog",
    "current_date",
    "current_role",
    "current_schema",
    "current_time",
    "current_timestamp",
    "current_user",
    "dec",
    "decimal",
    "default",
    "deferrable",
    "desc",
    "distinct",
    "do",
    "else",
    "end",
    "except",
    "exists",
    "extract",
    "false",
    "fetch",
    "float",
    "for",
    "foreign",
    "freeze",
    "from",
    "full",
    "grant",
    "graph_table",
    "greatest",
    "group",
    "grouping",
    "having",
    "ilike",
    "in",
    "initially",
    "inner",
    "inout",
    "int",
    "integer",
    "intersect",
    "interval",
    "into",
    "is",
    "isnull",
    "join",
    "json",
    "json_array",
    "json_arrayagg",
    "json_exists",
    "json_object",
    "json_objectagg",
    "json_query",
    "json_scalar",
    "json_serialize",
    "json_table",
    "json_value",
    "lateral",
    "leading",
    "least",
    "left",
    "like",
    "limit",
    "localtime",
    "localtimestamp",
    "merge_action",
    "national",
    "natural",
    "nchar",
    "none",
    "normalize",
    "not",
    "notnull",
    "null",
    "nullif",
    "numeric",
    "offset",
    "on",
    "only",
    "or",
    "order",
    "out",
    "outer",
    "overlaps",
    "overlay",
    "placing",
    "position",
    "precision",
    "primary",
    "real",
    "references",
    "returning",
    "right",
    "row",
    "select",
    "session_user",
    "setof",
    "similar",
    "smallint",
    "some",
    "substring",
    "symmetric",
    "system_user",
    "table",
    "tablesample",
    "then",
    "time",
    "timestamp",
    "to",
    "trailing",
    "treat",
    "trim",
    "true",
    "union",
    "unique",
    "user",
    "using",
    "values",
    "varchar",
    "variadic",
    "verbose",
    "when",
    "where",
    "window",
    "with",
    "xmlattributes",
    "xmlconcat",
    "xmlelement",
    "xmlexists",
    "xmlforest",
    "xmlnamespaces",
    "xmlparse",
    "xmlpi",
    "xmlroot",
    "xmlserialize",
    "xmltable",
];

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

    KEYWORDS_REQUIRING_QUOTES
        .binary_search(&identifier)
        .is_err()
}

#[cfg(test)]
mod tests {
    use super::escape_identifier;
    use super::KEYWORDS_REQUIRING_QUOTES;

    #[test]
    fn escape_identifier_matches_postgres_quote_identifier_cases() {
        let cases = [
            ("graphile_worker", "graphile_worker"),
            ("foo_bar1", "foo_bar1"),
            ("schema", "schema"),
            ("", "\"\""),
            ("Foo", "\"Foo\""),
            ("has-dash", "\"has-dash\""),
            ("123abc", "\"123abc\""),
            ("with\"quote", "\"with\"\"quote\""),
            ("select", "\"select\""),
            ("between", "\"between\""),
            ("éclair", "\"éclair\""),
        ];

        for (identifier, expected) in cases {
            assert_eq!(escape_identifier(identifier), expected);
        }
    }

    #[test]
    fn keywords_requiring_quotes_are_sorted() {
        assert!(KEYWORDS_REQUIRING_QUOTES
            .windows(2)
            .all(|window| window[0] < window[1]));
    }
}

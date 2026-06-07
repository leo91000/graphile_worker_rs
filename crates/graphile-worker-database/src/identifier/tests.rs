use super::escape_identifier;
use super::keywords::KEYWORDS_REQUIRING_QUOTES;

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

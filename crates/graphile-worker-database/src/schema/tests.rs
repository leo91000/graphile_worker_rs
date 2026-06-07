use super::Schema;

#[test]
fn schema_escapes_schema_tables_and_functions() {
    let schema = Schema::new("Case-Schema");

    assert_eq!(schema.escaped(), "\"Case-Schema\"");
    assert_eq!(
        schema.private_table("jobs"),
        "\"Case-Schema\"._private_jobs"
    );
    assert_eq!(schema.function("add_job"), "\"Case-Schema\".add_job");
}

#[test]
fn schema_quotes_private_table_and_function_identifiers_when_needed() {
    let schema = Schema::new("graphile_worker");

    assert_eq!(
        schema.private_table("jobs-dash"),
        "graphile_worker.\"_private_jobs-dash\""
    );
    assert_eq!(
        schema.function("function-with-dash"),
        "graphile_worker.\"function-with-dash\""
    );
}

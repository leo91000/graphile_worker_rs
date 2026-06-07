use graphile_worker::escape_identifier;

mod helpers;

#[tokio::test]
async fn escape_identifier_matches_postgres_format_identifier() {
    helpers::with_test_db(|test_db| async move {
        let cases = [
            "graphile_worker",
            "foo_bar1",
            "schema",
            "",
            "Foo",
            "has-dash",
            "123abc",
            "with\"quote",
            "select",
            "between",
            "éclair",
        ];

        for identifier in cases {
            let expected: String = sqlx::query_scalar("select format('%I', $1::text)")
                .bind(identifier)
                .fetch_one(&test_db.test_pool)
                .await
                .expect("Failed to escape identifier with PostgreSQL");

            assert_eq!(escape_identifier(identifier), expected);
        }
    })
    .await;
}

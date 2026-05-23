use graphile_worker::utils::escape_identifier;

#[tokio::test]
async fn escape_identifier_matches_postgres_format_identifier() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");
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
            .fetch_one(&pool)
            .await
            .expect("Failed to escape identifier with PostgreSQL");

        assert_eq!(escape_identifier(identifier), expected);
    }
}

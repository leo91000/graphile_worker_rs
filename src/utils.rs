use sqlx::{query_as, FromRow, PgExecutor};

use crate::errors::Result;

#[derive(FromRow)]
struct EscapeIdentifierRow {
    escaped_identifier: String,
}

pub async fn escape_identifier<'e>(
    executor: impl PgExecutor<'e>,
    identifier: &str,
) -> Result<String> {
    let result: EscapeIdentifierRow =
        query_as("select format('%I', $1::text) as escaped_identifier")
            .bind(identifier)
            .fetch_one(executor)
            .await?;

    Ok(result.escaped_identifier)
}

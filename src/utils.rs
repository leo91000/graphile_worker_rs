use sqlx::{query, Executor, Postgres};

use crate::errors::Result;

pub async fn escape_identifier<'e, E: Executor<'e, Database = Postgres>>(
    executor: E,
    identifier: &str,
) -> Result<String> {
    let escaped_identifier = query!(
        "select format('%I', $1::text) as escaped_identifier",
        identifier
    )
    .fetch_one(executor)
    .await?
    .escaped_identifier
    .unwrap();

    Ok(escaped_identifier)
}

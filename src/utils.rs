use graphile_worker_database::{DbExecutorArg, DbValue};

use crate::errors::Result;

struct EscapeIdentifierRow {
    escaped_identifier: String,
}

pub async fn escape_identifier(
    mut executor: impl DbExecutorArg,
    identifier: &str,
) -> Result<String> {
    let row = executor
        .fetch_one(
            "select format('%I', $1::text) as escaped_identifier",
            vec![DbValue::Text(identifier.to_string())].into(),
        )
        .await?;
    let result = EscapeIdentifierRow {
        escaped_identifier: row.try_get("escaped_identifier")?,
    };

    Ok(result.escaped_identifier)
}

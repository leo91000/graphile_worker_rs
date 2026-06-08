use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;

use crate::errors::Result;
use crate::schema_names::WorkerFunction;

pub async fn worker_heartbeat(
    mut executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    worker_id: &str,
    metadata: Option<serde_json::Value>,
) -> Result<()> {
    let schema = schema.into();
    let worker_heartbeat = WorkerFunction::WorkerHeartbeat.qualified(&schema);
    let sql = formatdoc!(
        r#"
            SELECT * FROM {worker_heartbeat}($1::text, $2::json);
        "#
    );

    executor
        .execute(
            &sql,
            vec![
                DbValue::Text(worker_id.to_string()),
                DbValue::JsonOpt(metadata),
            ]
            .into(),
        )
        .await?;

    Ok(())
}

pub async fn worker_deregister(
    mut executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    worker_id: &str,
) -> Result<()> {
    let schema = schema.into();
    let worker_deregister = WorkerFunction::WorkerDeregister.qualified(&schema);
    let sql = formatdoc!(
        r#"
            SELECT * FROM {worker_deregister}($1::text);
        "#
    );

    executor
        .execute(&sql, vec![DbValue::Text(worker_id.to_string())].into())
        .await?;

    Ok(())
}

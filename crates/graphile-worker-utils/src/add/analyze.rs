use graphile_worker_database::DbExecutorArg;
use indoc::formatdoc;
use tracing::debug;

use graphile_worker_queries::schema_names::PrivateTable;

use super::super::client::WorkerUtils;

const BULK_INSERT_ANALYZE_THRESHOLD: usize = 10_000;

pub(super) async fn analyze_jobs_after_large_batch(
    utils: &WorkerUtils,
    mut executor: impl DbExecutorArg,
    job_count: usize,
) {
    if job_count < BULK_INSERT_ANALYZE_THRESHOLD {
        return;
    }

    let jobs = PrivateTable::Jobs.qualified(&utils.schema);
    let sql = formatdoc!(
        r#"
            analyze {jobs};
        "#
    );

    if let Err(error) = executor
        .execute(&sql, graphile_worker_database::DbParams::new())
        .await
    {
        debug!(?error, "Failed to analyze jobs after large batch insert");
    }
}

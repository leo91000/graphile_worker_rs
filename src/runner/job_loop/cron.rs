use graphile_worker_crontab_runner::CronRunner;

use super::super::{Worker, WorkerRuntimeError};

pub(super) async fn run(worker: &Worker) -> Result<(), WorkerRuntimeError> {
    if worker.crontabs().is_empty() {
        return Ok(());
    }

    CronRunner::new(
        worker.database(),
        worker.schema(),
        worker.crontabs(),
        &worker.hooks,
    )
    .with_matchers(&worker.cron_matchers)
    .use_local_time(*worker.use_local_time())
    .run(worker.shutdown_signal.clone())
    .await?;

    Ok(())
}

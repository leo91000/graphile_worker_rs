use graphile_worker_crontab_runner::cron_main;

use super::super::{Worker, WorkerRuntimeError};

pub(super) async fn run(worker: &Worker) -> Result<(), WorkerRuntimeError> {
    if worker.crontabs().is_empty() {
        return Ok(());
    }

    cron_main(
        worker.database(),
        worker.schema(),
        worker.crontabs(),
        *worker.use_local_time(),
        worker.shutdown_signal.clone(),
        &worker.hooks,
    )
    .await?;

    Ok(())
}

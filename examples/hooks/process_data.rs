use graphile_worker::{IntoTaskHandlerResult, WorkerContext};
use graphile_worker_task_handler::TaskHandler;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub(super) struct ProcessData {
    pub(super) value: i32,
    #[serde(default)]
    pub(super) skip: bool,
    #[serde(default)]
    pub(super) force_fail: bool,
}

impl TaskHandler for ProcessData {
    const IDENTIFIER: &'static str = "process_data";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        println!("Processing data with value: {}", self.value);
        Ok::<(), String>(())
    }
}

#[derive(Serialize, Deserialize)]
struct TypedScheduleJob {
    message: String,
    #[serde(default)]
    transform: bool,
}

impl TaskHandler for TypedScheduleJob {
    const IDENTIFIER: &'static str = "typed_schedule_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use graphile_worker::{
    errors::GraphileWorkerError, runner::WorkerRuntimeError, JobComplete, JobFail,
};
use tokio::sync::Notify;

use super::*;

#[derive(Clone, Debug, Default)]
struct OutcomeGate {
    sibling_started: Arc<Notify>,
    finish_sibling: Arc<Notify>,
    sibling_finished: Arc<AtomicBool>,
}

#[derive(Serialize, Deserialize)]
struct OutcomeJob {
    poison: bool,
    fail: bool,
}

impl TaskHandler for OutcomeJob {
    const IDENTIFIER: &'static str = "run_once_outcome_error";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let gate = ctx.get_ext::<OutcomeGate>().unwrap();
        if self.poison {
            gate.sibling_started.notified().await;
        } else {
            gate.sibling_started.notify_one();
            gate.finish_sibling.notified().await;
            gate.sibling_finished.store(true, Ordering::SeqCst);
        }
        if self.fail {
            Err("handler failed")
        } else {
            Ok(())
        }
    }
}

async fn check_release_error(has_queue: bool, fail: bool) {
    helpers::with_test_db(move |db| async move {
        let gate = OutcomeGate::default();
        let completions = Arc::new(AtomicUsize::new(0));
        let failures = Arc::new(AtomicUsize::new(0));
        let worker = Rc::new(
            db.create_worker_options()
                .concurrency(2)
                .add_extension(gate.clone())
                .define_job::<OutcomeJob>()
                .on(JobComplete, {
                    let completions = completions.clone();
                    move |_| {
                        let completions = completions.clone();
                        async move { completions.fetch_add(1, Ordering::SeqCst); }
                    }
                })
                .on(JobFail, {
                    let failures = failures.clone();
                    move |_| {
                        let failures = failures.clone();
                        async move { failures.fetch_add(1, Ordering::SeqCst); }
                    }
                })
                .init().await.unwrap(),
        );
        let utils = worker.create_utils();
        let poisoned = utils.add_job(
            OutcomeJob { poison: true, fail },
            JobSpec { queue_name: has_queue.then(|| "poisoned".into()), ..Default::default() },
        ).await.unwrap();
        utils.add_job(OutcomeJob { poison: false, fail: false }, JobSpec::default()).await.unwrap();

        // A nontransactional sequence observes the rejected persistence attempt
        // even though the injected error rolls the statement back.
        sqlx::raw_sql(
            "CREATE SEQUENCE graphile_worker.outcome_attempts MINVALUE 0 START 0; \
             CREATE FUNCTION graphile_worker.reject_outcome() RETURNS trigger LANGUAGE plpgsql AS $$ \
             BEGIN \
               PERFORM nextval('graphile_worker.outcome_attempts'); \
               RAISE EXCEPTION USING ERRCODE='40001', MESSAGE='injected outcome failure'; \
             END $$;"
        ).execute(&db.test_pool).await.unwrap();
        let event = if fail {
            "UPDATE ON graphile_worker._private_jobs FOR EACH ROW \
             WHEN (OLD.locked_by IS NOT NULL AND NEW.locked_by IS NULL AND OLD.payload::jsonb->>'poison' = 'true')"
        } else {
            "DELETE ON graphile_worker._private_jobs FOR EACH ROW \
             WHEN (OLD.payload::jsonb->>'poison' = 'true')"
        };
        helpers::sql::safe_query(format!(
            "CREATE TRIGGER reject_outcome BEFORE {event} EXECUTE FUNCTION graphile_worker.reject_outcome()"
        )).execute(&db.test_pool).await.unwrap();

        let running_worker = worker.clone();
        let run = spawn_local(async move { running_worker.run_once().await });
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let called: bool = sqlx::query_scalar(
                    "SELECT is_called FROM graphile_worker.outcome_attempts"
                ).fetch_one(&db.test_pool).await.unwrap();
                if called { break; }
                sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("persistence fault must be reached while sibling is running");
        assert!(!run.is_finished(), "run_once must drain its sibling before returning");
        gate.finish_sibling.notify_one();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), run)
            .await.expect("run_once must finish").unwrap();
        assert!(gate.sibling_finished.load(Ordering::SeqCst));
        assert_eq!(completions.load(Ordering::SeqCst), 1, "only the healthy sibling emits completion");
        assert_eq!(failures.load(Ordering::SeqCst), 0, "unpersisted failure must not emit a hook");
        let error = result.expect_err("run_once must report failed outcome persistence");
        assert!(matches!(error, WorkerRuntimeError::ProcessJob(_)));
        assert!(error.to_string().contains(&format!("job '{}'", poisoned.id())));
        let mut source: &dyn std::error::Error = &error;
        loop {
            if let Some(GraphileWorkerError::SqlError(db_error)) = source.downcast_ref::<GraphileWorkerError>() {
                assert_eq!(db_error.code(), Some("40001"));
                assert!(db_error.to_string().contains("injected outcome failure"));
                break;
            }
            source = source.source().expect("database error must remain in the source chain");
        }
        let jobs = db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "healthy sibling must be persisted before return");
        assert_eq!(jobs[0].id, *poisoned.id());
        assert_eq!(jobs[0].attempts, 1);
        assert_eq!(jobs[0].locked_by.as_ref(), Some(worker.worker_id()));
        assert!(jobs[0].locked_at.is_some());
        assert!(jobs[0].last_error.is_none());
        if has_queue {
            let queues = db.get_job_queues().await;
            assert_eq!(queues.len(), 1);
            assert_eq!(queues[0].locked_by.as_ref(), Some(worker.worker_id()));
            assert!(queues[0].locked_at.is_some());
        }
    }).await;
}

#[tokio::test]
async fn direct_completion_error_drains_sibling_and_propagates() {
    check_release_error(false, false).await;
}

#[tokio::test]
async fn queued_completion_error_drains_sibling_and_propagates() {
    check_release_error(true, false).await;
}

#[tokio::test]
async fn direct_failure_error_drains_sibling_and_propagates() {
    check_release_error(false, true).await;
}

#[tokio::test]
async fn queued_failure_error_drains_sibling_and_propagates() {
    check_release_error(true, true).await;
}

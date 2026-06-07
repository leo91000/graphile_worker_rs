use super::*;

#[tokio::test]
async fn cron_runner_schedules_job_on_tick() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let initial_time = Local::now();
        let clock = Arc::new(MockClock::new(initial_time));
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();

        let crontabs = parse_crontab("* * * * * test_task").expect("Failed to parse crontab");

        let hooks = HookRegistry::default();
        let database = test_db.database.clone();
        let clock_for_runner = clock.clone();
        let runner_handle = spawn_local(async move {
            CronRunner::new(&database, "graphile_worker", &crontabs, &hooks)
                .with_clock(clock_for_runner)
                .run(shutdown_signal)
                .await
        });

        tokio::task::yield_now().await;

        clock.advance(Duration::minutes(2));

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            if !jobs.is_empty() {
                assert_eq!(jobs[0].task_identifier, "test_task");
                break;
            }
            if start.elapsed().as_secs() > 5 {
                panic!("Job should have been scheduled by now");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        shutdown_notify.notify_one();
        runner_handle.await.ok();
    })
    .await;
}
#[tokio::test]
async fn cron_runner_catches_up_after_clock_jump() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let initial_time = Local::now();
        let clock = Arc::new(MockClock::new(initial_time));
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();

        let crontabs = parse_crontab("* * * * * catchup_task").expect("Failed to parse crontab");

        let hooks = HookRegistry::default();
        let database = test_db.database.clone();
        let clock_for_runner = clock.clone();
        let runner_handle = spawn_local(async move {
            CronRunner::new(&database, "graphile_worker", &crontabs, &hooks)
                .with_clock(clock_for_runner)
                .run(shutdown_signal)
                .await
        });

        tokio::task::yield_now().await;

        clock.advance(Duration::minutes(5));

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            if jobs.len() >= 4 {
                assert!(jobs.iter().all(|j| j.task_identifier == "catchup_task"));
                break;
            }
            if start.elapsed().as_secs() > 5 {
                let jobs = test_db.get_jobs().await;
                panic!(
                    "Expected at least 4 jobs after 5 minute clock jump, got {}",
                    jobs.len()
                );
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        shutdown_notify.notify_one();
        runner_handle.await.ok();
    })
    .await;
}
#[tokio::test]
async fn cron_runner_calls_hooks() {
    use graphile_worker::{CronJobScheduled, CronTick, Plugin};
    use std::sync::atomic::{AtomicU32, Ordering};

    static TICK_COUNT: AtomicU32 = AtomicU32::new(0);
    static SCHEDULED_COUNT: AtomicU32 = AtomicU32::new(0);

    struct CronHooksPlugin;

    impl Plugin for CronHooksPlugin {
        fn register(self, hooks: &mut HookRegistry) {
            hooks.on(CronTick, |_ctx| async {
                TICK_COUNT.fetch_add(1, Ordering::SeqCst);
            });

            hooks.on(CronJobScheduled, |_ctx| async {
                SCHEDULED_COUNT.fetch_add(1, Ordering::SeqCst);
            });
        }
    }

    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let initial_time = Local::now();
        let clock = Arc::new(MockClock::new(initial_time));
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();

        let crontabs = parse_crontab("* * * * * hook_task").expect("Failed to parse crontab");

        let hooks = HookRegistry::default().with_plugin(CronHooksPlugin);

        let database = test_db.database.clone();
        let clock_for_runner = clock.clone();
        let runner_handle = spawn_local(async move {
            CronRunner::new(&database, "graphile_worker", &crontabs, &hooks)
                .with_clock(clock_for_runner)
                .run(shutdown_signal)
                .await
        });

        tokio::task::yield_now().await;

        clock.advance(Duration::minutes(3));

        let start = Instant::now();
        loop {
            let tick_count = TICK_COUNT.load(Ordering::SeqCst);
            let scheduled_count = SCHEDULED_COUNT.load(Ordering::SeqCst);
            if tick_count >= 2 && scheduled_count >= 2 {
                break;
            }
            if start.elapsed().as_secs() > 5 {
                panic!(
                    "Expected hooks to be called. tick_count={}, scheduled_count={}",
                    tick_count, scheduled_count
                );
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        shutdown_notify.notify_one();
        runner_handle.await.ok();
    })
    .await;
}
#[tokio::test]
async fn cron_runner_shutdown_cleanly() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let initial_time = Local::now();
        let clock = Arc::new(MockClock::new(initial_time));
        let (shutdown_signal, shutdown_notify) = create_shutdown_signal();

        let crontabs = parse_crontab("* * * * * shutdown_task").expect("Failed to parse crontab");

        let hooks = HookRegistry::default();
        let database = test_db.database.clone();
        let clock_for_runner = clock.clone();
        let runner_handle = spawn_local(async move {
            CronRunner::new(&database, "graphile_worker", &crontabs, &hooks)
                .with_clock(clock_for_runner)
                .run(shutdown_signal)
                .await
        });

        tokio::task::yield_now().await;

        shutdown_notify.notify_one();

        let result = runner_handle.await;
        assert!(result.is_ok(), "Runner should complete without error");
        assert!(
            result.unwrap().is_ok(),
            "Runner should return Ok on shutdown"
        );
    })
    .await;
}

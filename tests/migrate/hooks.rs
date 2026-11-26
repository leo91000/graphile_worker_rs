use std::time::Duration;

use graphile_worker::{MigrationHooks, MigrationPhase, WorkerOptions, WorkerUtils};
use graphile_worker_migrations::migrate_with_hooks;
use tokio::sync::oneshot;
use tokio::time::timeout;

use super::*;

const DEADLINE: Duration = Duration::from_secs(15);

#[tokio::test]
async fn hooks_share_the_session_with_worker_initialization_and_every_revision() {
    with_test_db(|test_db| async move {
        let hooks = MigrationHooks::new()
            .on_event(|event| {
                Box::pin(async move {
                    let executor = event.executor;
                    assert!(event.postgres_version >= 120000);
                    let pid: i32 = executor
                        .fetch_one("select pg_backend_pid() as pid", DbParams::new())
                        .await?
                        .try_get("pid")?;
                    match event.phase {
                        MigrationPhase::BeforeBootstrap => {
                            assert_eq!(event.scratchpad.insert(pid), None);
                            executor
                                .execute("create temp table migration_pids (pid int)", DbParams::new())
                                .await?;
                        }
                        MigrationPhase::AfterBootstrap => {
                            assert_eq!(event.scratchpad.get::<i32>(), Some(&pid));
                            let function = event.schema.identifier("record_backend");
                            let ledger = event.schema.identifier("migrations");
                            executor
                                .execute(
                                    &format!("create function {function}() returns trigger language plpgsql as $$
                                        begin insert into pg_temp.migration_pids values (pg_backend_pid()); return new; end;
                                    $$"),
                                    DbParams::new(),
                                )
                                .await?;
                            executor
                                .execute(
                                    &format!("create trigger record_backend before insert on {ledger}
                                        for each row execute function {function}()"),
                                    DbParams::new(),
                                )
                                .await?;
                        }
                        MigrationPhase::BeforeMigrate => {
                            assert_eq!(event.scratchpad.get::<i32>(), Some(&pid));
                        }
                        MigrationPhase::AfterMigrate => {
                            let row = executor
                                .fetch_one(
                                    "select count(*) as revisions, min(pid) as first, max(pid) as last from pg_temp.migration_pids",
                                    DbParams::new(),
                                )
                                .await?;
                            assert_eq!(row.try_get::<i64>("revisions")?, GRAPHILE_WORKER_MIGRATIONS.len() as i64);
                            assert_eq!(row.try_get::<i32>("first")?, pid);
                            assert_eq!(row.try_get::<i32>("last")?, pid);
                        }
                        _ => panic!("unexpected phase: {:?}", event.phase),
                    }
                    Ok(())
                })
            })
            .on_event(|event| {
                Box::pin(async move {
                    assert!(event.scratchpad.get::<i32>().is_some());
                    Ok(())
                })
            });
        let worker = timeout(
            DEADLINE,
            WorkerOptions::default()
                .database(&test_db.database)
                .schema("with Migration Hooks")
                .migration_hooks(hooks)
                .init(),
        )
        .await
        .expect("worker initialization timed out")
        .expect("migrations and worker registration should succeed");

        let hooks = MigrationHooks::new().on_event(|event| {
            Box::pin(async move {
                match event.phase {
                    MigrationPhase::BeforeMigrate => assert!(event.scratchpad.is_empty()),
                    MigrationPhase::AfterMigrate => {
                        let row = event.executor
                            .fetch_one(
                                "select to_regclass('pg_temp.migration_pids') is null as clean",
                                DbParams::new(),
                            )
                            .await?;
                        assert!(row.try_get::<bool>("clean")?);
                    }
                    _ => panic!("unexpected phase during no-op migration: {:?}", event.phase),
                }
                Ok(())
            })
        });
        timeout(DEADLINE, worker.create_utils().migrate_with_hooks(&hooks))
            .await
            .expect("migration timed out")
            .expect("already-current schema should still run migrate callbacks");
    })
    .await;
}

#[tokio::test]
async fn failure_hook_runs_after_rollback_and_retry_resumes_from_committed_revision() {
    with_test_db(|test_db| async move {
        let hooks = MigrationHooks::new().on_event(|event| {
            Box::pin(async move {
                let executor = event.executor;
                match event.phase {
                    MigrationPhase::AfterBootstrap => {
                        let ledger = event.schema.identifier("migrations");
                        let function = event.schema.identifier("fail_revision");
                        executor.execute("create temp table migration_attempts (id int)", DbParams::new()).await?;
                        executor
                            .execute(
                                &format!("create function {function}() returns trigger language plpgsql as $$
                                    begin
                                        insert into pg_temp.migration_attempts values (new.id);
                                        if new.id = 2 then raise exception 'intentional revision failure'; end if;
                                        return new;
                                    end;
                                $$"),
                                DbParams::new(),
                            )
                            .await?;
                        executor
                            .execute(
                                &format!("create trigger fail_revision before insert on {ledger}
                                    for each row execute function {function}()"),
                                DbParams::new(),
                            )
                            .await?;
                    }
                    MigrationPhase::RevisionFailed { revision, error } => {
                        assert_eq!(revision, 2);
                        assert_eq!(error.code(), Some("P0001"));
                        // This statement requires both a non-aborted transaction
                        // and continued access to the original session's temp table.
                        let row = executor
                            .fetch_one(
                                "select json_agg(id order by id) as ids from pg_temp.migration_attempts",
                                DbParams::new(),
                            )
                            .await?;
                        assert_eq!(row.try_get::<serde_json::Value>("ids")?, json!([1]));
                        let ledger = event.schema.identifier("migrations");
                        executor
                            .execute(&format!("drop trigger fail_revision on {ledger}"), DbParams::new())
                            .await?;
                    }
                    MigrationPhase::AfterMigrate => panic!("failed run cannot reach AfterMigrate"),
                    _ => (),
                }
                Ok(())
            })
        });
        let error = timeout(
            DEADLINE,
            migrate_with_hooks(&test_db.database, "graphile_worker", &hooks),
        )
        .await
        .expect("failed revision timed out")
        .expect_err("migration should retain the injected error");
        assert!(matches!(error, MigrateError::SqlError(error) if error.code() == Some("P0001")));
        let ledger = test_db.get_migrations().await;
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].id, 1);

        timeout(
            DEADLINE,
            WorkerUtils::new(&test_db.database, "graphile_worker").migrate(),
        )
        .await
        .expect("migration retry timed out")
        .expect("migration retry should succeed");
        let ledger = test_db.get_migrations().await;
        assert_eq!(ledger.len(), GRAPHILE_WORKER_MIGRATIONS.len());
        assert_eq!(ledger[0].id, 1);
    })
    .await;
}

#[tokio::test]
async fn cancelling_migration_hooks_releases_the_session_and_its_locks() {
    with_test_db(|test_db| async move {
        let (send, receive) = oneshot::channel();
        let send = std::sync::Mutex::new(Some(send));
        let database = test_db.database.clone();
        let worker = tokio::spawn(async move {
            let hooks = MigrationHooks::new().on_event(move |event| {
                let send = send.lock().unwrap().take().expect("first hook");
                Box::pin(async move {
                    assert!(matches!(event.phase, MigrationPhase::BeforeBootstrap));
                    event
                        .executor
                        .execute("select pg_advisory_lock(87)", DbParams::new())
                        .await?;
                    send.send(()).expect("test is waiting for the hook");
                    std::future::pending().await
                })
            });
            migrate_with_hooks(database, "graphile_worker", &hooks).await
        });
        timeout(DEADLINE, receive)
            .await
            .expect("before-bootstrap callback timed out")
            .expect("before-bootstrap callback failed");
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());

        // The lock is scoped to the isolated test database. The waiter proves
        // the pool did not return the original session with the lock still held.
        timeout(
            DEADLINE,
            query("select pg_advisory_xact_lock(87)").execute(&test_db.test_pool),
        )
        .await
        .expect("cancelled migration left an advisory lock held")
        .expect("fixture should be able to acquire the lock");
        timeout(DEADLINE, migrate(&test_db.database, "graphile_worker"))
            .await
            .expect("migration retry timed out")
            .expect("migration retry should succeed");
    })
    .await;
}

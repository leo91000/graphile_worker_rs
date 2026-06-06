use indoc::indoc;

use super::GraphileWorkerMigration;

pub const M000020_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000020",
    is_breaking: false,
    stmts: &[
        indoc! {r#"
            CREATE TABLE :GRAPHILE_WORKER_SCHEMA._private_workers (
                id text PRIMARY KEY,
                last_heartbeat_at timestamptz NOT NULL,
                started_at timestamptz NOT NULL DEFAULT now(),
                metadata jsonb
            );
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.worker_heartbeat(
                worker_id text,
                metadata json DEFAULT NULL::json
            ) RETURNS void
            LANGUAGE sql
            AS $$
                INSERT INTO :GRAPHILE_WORKER_SCHEMA._private_workers AS workers (id, last_heartbeat_at, metadata)
                VALUES (worker_heartbeat.worker_id, now(), metadata::jsonb)
                ON CONFLICT (id) DO UPDATE
                SET
                    last_heartbeat_at = now(),
                    metadata = COALESCE(EXCLUDED.metadata, workers.metadata);
            $$;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.worker_deregister(worker_id text) RETURNS void
            LANGUAGE sql
            AS $$
                DELETE FROM :GRAPHILE_WORKER_SCHEMA._private_workers
                WHERE id = worker_deregister.worker_id;
            $$;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.list_stale_workers(
                stale_threshold interval
            ) RETURNS TABLE(worker_id text)
            LANGUAGE sql
            AS $$
                SELECT workers.id AS worker_id
                FROM :GRAPHILE_WORKER_SCHEMA._private_workers AS workers
                WHERE workers.last_heartbeat_at < now() - stale_threshold;
            $$;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.list_orphan_locked_workers(
                stale_threshold interval
            ) RETURNS TABLE(worker_id text)
            LANGUAGE sql
            AS $$
                SELECT DISTINCT locks.locked_by AS worker_id
                FROM (
                    SELECT jobs.locked_by, jobs.locked_at
                    FROM :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
                    WHERE jobs.locked_by IS NOT NULL
                    UNION ALL
                    SELECT job_queues.locked_by, job_queues.locked_at
                    FROM :GRAPHILE_WORKER_SCHEMA._private_job_queues AS job_queues
                    WHERE job_queues.locked_by IS NOT NULL
                ) AS locks
                WHERE locks.locked_at < now() - stale_threshold
                AND NOT EXISTS (
                    SELECT 1
                    FROM :GRAPHILE_WORKER_SCHEMA._private_workers AS workers
                    WHERE workers.id = locks.locked_by
                );
            $$;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.recover_dead_worker_jobs(
                worker_ids text[],
                recovery_delay interval DEFAULT interval '0 seconds'
            ) RETURNS integer
            LANGUAGE plpgsql
            AS $$
            DECLARE
                recovered_count integer := 0;
                queue_count integer := 0;
            BEGIN
                WITH j AS (
                    UPDATE :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
                    SET
                        attempts = GREATEST(0, jobs.attempts - 1),
                        locked_by = NULL,
                        locked_at = NULL,
                        run_at = GREATEST(jobs.run_at, now() + recovery_delay),
                        last_error = COALESCE(jobs.last_error, 'Job recovered after worker interruption'),
                        updated_at = now()
                    WHERE jobs.locked_by = ANY(worker_ids)
                    RETURNING jobs.job_queue_id
                )
                SELECT COUNT(*) INTO recovered_count FROM j;

                UPDATE :GRAPHILE_WORKER_SCHEMA._private_job_queues AS job_queues
                SET locked_by = NULL, locked_at = NULL
                WHERE job_queues.locked_by = ANY(worker_ids);

                GET DIAGNOSTICS queue_count = ROW_COUNT;
                RETURN recovered_count + queue_count;
            END;
            $$;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.delete_stale_workers(
                worker_ids text[]
            ) RETURNS void
            LANGUAGE sql
            AS $$
                DELETE FROM :GRAPHILE_WORKER_SCHEMA._private_workers AS workers
                WHERE workers.id = ANY(worker_ids);
            $$;
        "#},
    ],
};
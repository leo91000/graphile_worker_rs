DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.jobs__increase_job_queue_count();

-- graphile-worker-rs:statement

DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.jobs__decrease_job_queue_count();

-- graphile-worker-rs:statement

DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.tg__update_timestamp();

-- graphile-worker-rs:statement

CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.force_unlock_workers(worker_ids text[]) RETURNS void AS $$
UPDATE :GRAPHILE_WORKER_SCHEMA.jobs
SET locked_at = null, locked_by = null
WHERE locked_by = ANY(worker_ids);
UPDATE :GRAPHILE_WORKER_SCHEMA.job_queues
SET locked_at = null, locked_by = null
WHERE locked_by = ANY(worker_ids);
$$ LANGUAGE sql VOLATILE;

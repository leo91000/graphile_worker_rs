-- Explicitly distinguish replaced jobs from ordinary final attempts.
CREATE TABLE :GRAPHILE_WORKER_SCHEMA._private_job_retirements (
    id bigint PRIMARY KEY REFERENCES :GRAPHILE_WORKER_SCHEMA._private_jobs(id) ON DELETE CASCADE
);

-- graphile-worker-rs:statement

-- Retain ownership until the handler releases the job and its named queue.
CREATE OR REPLACE FUNCTION :GRAPHILE_WORKER_SCHEMA.add_jobs(specs :GRAPHILE_WORKER_SCHEMA.job_spec[], job_key_preserve_run_at boolean DEFAULT false) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
LANGUAGE plpgsql
AS $$
declare
    locked_key text;
begin
    for locked_key in
        select distinct spec.job_key
        from unnest(specs) spec
        where spec.job_key is not null
        order by spec.job_key
    loop
        perform pg_advisory_xact_lock(hashtext(locked_key));
    end loop;

    perform 1
    from :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs
    where jobs.key = any(
        array(
            select distinct spec.job_key
            from unnest(specs) spec
            where spec.job_key is not null
        )
    )
    for update;

    insert into :GRAPHILE_WORKER_SCHEMA._private_tasks as tasks (identifier)
    select distinct spec.identifier
    from unnest(specs) spec
    where not exists (
        select 1 from :GRAPHILE_WORKER_SCHEMA._private_tasks as existing
        where existing.identifier = spec.identifier
    )
    on conflict do nothing;

    insert into :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues (queue_name)
    select distinct spec.queue_name
    from unnest(specs) spec
    where spec.queue_name is not null
    and not exists (
        select 1 from :GRAPHILE_WORKER_SCHEMA._private_job_queues as existing
        where existing.queue_name = spec.queue_name
    )
    on conflict do nothing;

    with retired as (
        update :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs
        set
            key = null,
            attempts = jobs.max_attempts,
            updated_at = now()
        from unnest(specs) spec
        where spec.job_key is not null
        and jobs.key = spec.job_key
        and is_available is not true
        returning jobs.id
    )
    insert into :GRAPHILE_WORKER_SCHEMA._private_job_retirements(id)
    select id from retired
    on conflict do nothing;

    perform pg_notify('jobs:insert', '{"r":' || random()::text || ',"count":' || array_length(specs, 1)::text || '}');

    return query insert into :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs (
        job_queue_id,
        task_id,
        payload,
        run_at,
        max_attempts,
        key,
        priority,
        flags
    )
        select
            job_queues.id,
            tasks.id,
            coalesce(spec.payload, '{}'::json),
            coalesce(spec.run_at, now()),
            coalesce(spec.max_attempts, 25),
            spec.job_key,
            coalesce(spec.priority, 0),
            (
                select jsonb_object_agg(flag, true)
                from unnest(spec.flags) as item(flag)
            )
        from unnest(specs) spec
        inner join :GRAPHILE_WORKER_SCHEMA._private_tasks as tasks
        on tasks.identifier = spec.identifier
        left join :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues
        on job_queues.queue_name = spec.queue_name
    on conflict (key) do update set
        job_queue_id = excluded.job_queue_id,
        task_id = excluded.task_id,
        payload =
            case
            when json_typeof(jobs.payload) = 'array' and json_typeof(excluded.payload) = 'array' then
                (jobs.payload::jsonb || excluded.payload::jsonb)::json
            else
                excluded.payload
            end,
        max_attempts = excluded.max_attempts,
        run_at = (case
            when job_key_preserve_run_at is true and jobs.attempts = 0 then jobs.run_at
            else excluded.run_at
        end),
        priority = excluded.priority,
        revision = jobs.revision + 1,
        flags = excluded.flags,
        attempts = 0,
        last_error = null,
        updated_at = now()
    where jobs.locked_at is null
    returning *;
end;
$$;


-- graphile-worker-rs:statement

CREATE OR REPLACE FUNCTION :GRAPHILE_WORKER_SCHEMA.recover_dead_worker_jobs(
    worker_ids text[],
    recovery_delay interval DEFAULT interval '0 seconds'
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
    recovered_count integer := 0;
BEGIN
    -- Take row locks before the statement that reads retirement markers.
    PERFORM 1 FROM :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    WHERE jobs.locked_by = ANY(worker_ids)
    ORDER BY jobs.id
    FOR UPDATE;

    WITH j AS (
        UPDATE :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
        SET
            attempts = CASE WHEN EXISTS (
                SELECT 1 FROM :GRAPHILE_WORKER_SCHEMA._private_job_retirements AS retired
                WHERE retired.id = jobs.id
            ) THEN jobs.attempts
                ELSE GREATEST(0, jobs.attempts - 1) END,
            locked_by = NULL,
            locked_at = NULL,
            run_at = GREATEST(jobs.run_at, now() + recovery_delay),
            last_error = 'Job recovered after worker interruption',
            updated_at = now()
        WHERE jobs.locked_by = ANY(worker_ids)
        RETURNING jobs.job_queue_id
    )
    SELECT COUNT(*) INTO recovered_count FROM j;

    UPDATE :GRAPHILE_WORKER_SCHEMA._private_job_queues AS job_queues
    SET locked_by = NULL, locked_at = NULL
    WHERE job_queues.locked_by = ANY(worker_ids);

    RETURN recovered_count;
END;
$$;


-- graphile-worker-rs:statement

CREATE OR REPLACE FUNCTION :GRAPHILE_WORKER_SCHEMA.reschedule_jobs(job_ids bigint[], run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, priority integer DEFAULT NULL::integer, attempts integer DEFAULT NULL::integer, max_attempts integer DEFAULT NULL::integer) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
LANGUAGE plpgsql
AS $$
BEGIN
    -- Explicit revival must see and clear a concurrently committed retirement.
    PERFORM 1 FROM :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    WHERE jobs.id = ANY(job_ids)
    AND (jobs.locked_at IS NULL OR jobs.locked_at < now() - interval '4 hours')
    ORDER BY jobs.id
    FOR UPDATE;

    RETURN QUERY WITH updated AS (
    UPDATE :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    SET
        run_at = COALESCE(reschedule_jobs.run_at, jobs.run_at),
        priority = COALESCE(reschedule_jobs.priority::smallint, jobs.priority),
        attempts = COALESCE(reschedule_jobs.attempts::smallint, jobs.attempts),
        max_attempts = COALESCE(reschedule_jobs.max_attempts::smallint, jobs.max_attempts),
        updated_at = now()
    WHERE id = ANY(job_ids)
    AND (
        locked_at IS NULL
    OR
        locked_at < NOW() - interval '4 hours'
    )
    RETURNING *
    ), cleared AS (
        DELETE FROM :GRAPHILE_WORKER_SCHEMA._private_job_retirements AS retired
        USING updated
        WHERE retired.id = updated.id
    )
    SELECT * FROM updated;
END;
$$;

-- graphile-worker-rs:statement

CREATE OR REPLACE FUNCTION :GRAPHILE_WORKER_SCHEMA.remove_job(job_key text) RETURNS :GRAPHILE_WORKER_SCHEMA._private_jobs
LANGUAGE plpgsql STRICT
AS $$
declare
    v_job :GRAPHILE_WORKER_SCHEMA._private_jobs;
begin
    -- Delete job if not locked
    DELETE FROM :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    WHERE key = job_key
    AND (
        locked_at IS NULL
    OR
        locked_at < NOW() - interval '4 hours'
    )
    RETURNING * INTO v_job;
    IF NOT (v_job IS NULL) THEN
        RETURN v_job;
    END IF;
    -- Otherwise prevent job from retrying, and clear the key
    UPDATE :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    SET
        key = NULL,
        attempts = jobs.max_attempts,
        updated_at = now()
    WHERE key = job_key
    RETURNING * INTO v_job;
    IF v_job.id IS NOT NULL THEN
        INSERT INTO :GRAPHILE_WORKER_SCHEMA._private_job_retirements(id)
        VALUES (v_job.id) ON CONFLICT DO NOTHING;
    END IF;
    RETURN v_job;
end;
$$;

-- graphile-worker-rs:statement

CREATE OR REPLACE FUNCTION :GRAPHILE_WORKER_SCHEMA.permanently_fail_jobs(job_ids bigint[], error_message text DEFAULT NULL::text) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
LANGUAGE sql
AS $$
    WITH retired AS (
    UPDATE :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    SET
        last_error = COALESCE(error_message, 'Manually marked as failed'),
        attempts = max_attempts,
        updated_at = now()
    WHERE id = ANY(job_ids)
    AND (
        locked_at IS NULL
    OR
        locked_at < NOW() - interval '4 hours'
    )
    RETURNING *
    ), marked AS (
        INSERT INTO :GRAPHILE_WORKER_SCHEMA._private_job_retirements(id)
        SELECT id FROM retired
        ON CONFLICT DO NOTHING
    )
    SELECT * FROM retired;
$$;

-- graphile-worker-rs:statement

-- Lock first, then read retirement state in a fresh READ COMMITTED snapshot.
-- A single UPDATE with a marker subquery can miss a retirement committed while
-- the UPDATE was waiting for the job row lock.
CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA._private_return_jobs(
    worker_id text,
    job_ids bigint[],
    recovery_delay_ms bigint DEFAULT NULL,
    recovery_error text DEFAULT NULL,
    update_recovery_metadata boolean DEFAULT false
) RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM 1 FROM :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
    WHERE jobs.id = ANY(job_ids) AND jobs.locked_by = worker_id
    ORDER BY jobs.id
    FOR UPDATE;

    WITH returned AS (
        UPDATE :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
        SET
            attempts = CASE WHEN EXISTS (
                SELECT 1 FROM :GRAPHILE_WORKER_SCHEMA._private_job_retirements AS retired
                WHERE retired.id = jobs.id
            ) THEN jobs.attempts ELSE GREATEST(0, jobs.attempts - 1) END,
            locked_by = NULL,
            locked_at = NULL,
            run_at = CASE WHEN update_recovery_metadata AND recovery_delay_ms IS NOT NULL
                THEN GREATEST(jobs.run_at, now() + recovery_delay_ms * interval '1 millisecond')
                ELSE jobs.run_at END,
            last_error = CASE WHEN update_recovery_metadata
                THEN COALESCE(recovery_error, jobs.last_error) ELSE jobs.last_error END,
            updated_at = CASE WHEN update_recovery_metadata THEN now() ELSE jobs.updated_at END
        WHERE jobs.id = ANY(job_ids) AND jobs.locked_by = worker_id
        RETURNING jobs.job_queue_id
    )
    UPDATE :GRAPHILE_WORKER_SCHEMA._private_job_queues AS queues
    SET locked_by = NULL, locked_at = NULL
    FROM returned
    WHERE queues.id = returned.job_queue_id AND queues.locked_by = worker_id;
END;
$$;

pub const M000014_MIGRATION: &[&str] = &[
    // Drop the existing 'add_job' function
    r#"
        DROP FUNCTION :ARCHIMEDES_SCHEMA.add_job;
    "#,
    // Create the new 'add_job' function with changed parameters
    r#"
        CREATE FUNCTION :ARCHIMEDES_SCHEMA.add_job(identifier text, payload json DEFAULT NULL::json, queue_name text DEFAULT NULL::text, run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, max_attempts int DEFAULT NULL::int, job_key text DEFAULT NULL::text, priority int DEFAULT NULL::int, flags text[] DEFAULT NULL::text[], job_key_mode text DEFAULT 'replace'::text) RETURNS :ARCHIMEDES_SCHEMA.jobs
        LANGUAGE plpgsql
        AS $$
        declare
            v_job :ARCHIMEDES_SCHEMA.jobs;
        begin
            if (job_key is null or job_key_mode is null or job_key_mode in ('replace', 'preserve_run_at')) then
                select * into v_job
                from :ARCHIMEDES_SCHEMA.add_jobs(
                    ARRAY[(
                        identifier,
                        payload,
                        queue_name,
                        run_at,
                        max_attempts::smallint,
                        job_key,
                        priority::smallint,
                        flags
                    ):::ARCHIMEDES_SCHEMA.job_spec],
                    (job_key_mode = 'preserve_run_at')
                )
                limit 1;
                return v_job;
            elsif job_key_mode = 'unsafe_dedupe' then
                insert into :ARCHIMEDES_SCHEMA.tasks (identifier)
                values (identifier)
                on conflict do nothing;
                if queue_name is not null then
                    insert into :ARCHIMEDES_SCHEMA.job_queues (queue_name)
                    values (queue_name)
                    on conflict do nothing;
                end if;
                insert into :ARCHIMEDES_SCHEMA.jobs (
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
                    coalesce(payload, '{}'::json),
                    coalesce(run_at, now()),
                    coalesce(max_attempts::smallint, 25::smallint),
                    job_key,
                    coalesce(priority::smallint, 0::smallint),
                    (
                        select jsonb_object_agg(flag, true)
                        from unnest(flags) as item(flag)
                    )
                from :ARCHIMEDES_SCHEMA.tasks
                left join :ARCHIMEDES_SCHEMA.job_queues
                on job_queues.queue_name = queue_name
                where tasks.identifier = identifier
                on conflict (key)
                do update set
                    revision = jobs.revision + 1,
                    updated_at = now()
                returning *
                into v_job;
                return v_job;
            else
                raise exception 'Invalid job_key_mode value, expected ''replace'', ''preserve_run_at'' or ''unsafe_dedupe''.' using errcode = 'GWBKM';
            end if;
        end;
        $$;
    "#,
    // Drop the existing 'reschedule_jobs' function
    r#"
        DROP FUNCTION :ARCHIMEDES_SCHEMA.reschedule_jobs;
    "#,
    // Create the new 'reschedule_jobs' function with changed parameters
    r#"
        CREATE FUNCTION :ARCHIMEDES_SCHEMA.reschedule_jobs(job_ids bigint[], run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, priority int DEFAULT NULL::int, attempts int DEFAULT NULL::int, max_attempts int DEFAULT NULL::int) RETURNS SETOF :ARCHIMEDES_SCHEMA.jobs
        LANGUAGE sql
        AS $$
            UPDATE :ARCHIMEDES_SCHEMA.jobs
            SET
                run_at = coalesce(run_at, jobs.run_at),
                priority = coalesce(priority::smallint, jobs.priority),
                attempts = coalesce(attempts::smallint, jobs.attempts),
                max_attempts = coalesce(max_attempts::smallint, jobs.max_attempts),
                updated_at = now()
            WHERE id = ANY(job_ids)
            AND (
                locked_at is null
            OR
                locked_at < NOW() - interval '4 hours'
            )
            RETURNING *;
        $$;
    "#,
];

use indoc::indoc;

use super::GraphileWorkerMigration;

pub const M000016_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000016",
    is_breaking: true,
    stmts: &[
        // Rename tables
        indoc! {r#"
            ALTER TABLE :GRAPHILE_WORKER_SCHEMA.jobs RENAME TO _private_jobs;
        "#},
        indoc! {r#"
            ALTER TABLE :GRAPHILE_WORKER_SCHEMA.job_queues RENAME TO _private_job_queues;
        "#},
        indoc! {r#"
            ALTER TABLE :GRAPHILE_WORKER_SCHEMA.tasks RENAME TO _private_tasks;
        "#},
        indoc! {r#"
            ALTER TABLE :GRAPHILE_WORKER_SCHEMA.known_crontabs RENAME TO _private_known_crontabs;
        "#},
        // Drop and create new 'add_job' function
        indoc! {r#"
            DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.add_job;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.add_job(identifier text, payload json DEFAULT NULL::json, queue_name text DEFAULT NULL::text, run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, max_attempts integer DEFAULT NULL::integer, job_key text DEFAULT NULL::text, priority integer DEFAULT NULL::integer, flags text[] DEFAULT NULL::text[], job_key_mode text DEFAULT 'replace'::text) RETURNS :GRAPHILE_WORKER_SCHEMA._private_jobs
            LANGUAGE plpgsql
            AS $$
            declare
                v_job :GRAPHILE_WORKER_SCHEMA._private_jobs;
            begin
                if (job_key is null or job_key_mode is null or job_key_mode in ('replace', 'preserve_run_at')) then
                    select * into v_job
                    from :GRAPHILE_WORKER_SCHEMA.add_jobs(
                        ARRAY[(
                            identifier,
                            payload,
                            queue_name,
                            run_at,
                            max_attempts::smallint,
                            job_key,
                            priority::smallint,
                            flags
                        ):::GRAPHILE_WORKER_SCHEMA.job_spec],
                        (job_key_mode = 'preserve_run_at')
                    )
                    limit 1;
                    return v_job;
                elsif job_key_mode = 'unsafe_dedupe' then
                    -- Ensure all the tasks exist
                    insert into :GRAPHILE_WORKER_SCHEMA._private_tasks as tasks (identifier)
                    values (identifier)
                    on conflict do nothing;
                    -- Ensure all the queues exist
                    if queue_name is not null then
                        insert into :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues (queue_name)
                        values (queue_name)
                        on conflict do nothing;
                    end if;
                    -- Insert job, but if one already exists then do nothing
                    insert into :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs (
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
                    from :GRAPHILE_WORKER_SCHEMA._private_tasks as tasks
                    left join :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues
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
        "#},
        // Drop and create new 'add_jobs' function
        indoc! {r#"
            DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.add_jobs;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.add_jobs(specs :GRAPHILE_WORKER_SCHEMA.job_spec[], job_key_preserve_run_at boolean DEFAULT false) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
            LANGUAGE plpgsql
            AS $$
            begin
                -- Ensure all the tasks exist
                insert into :GRAPHILE_WORKER_SCHEMA._private_tasks as tasks (identifier)
                select distinct spec.identifier
                from unnest(specs) spec
                on conflict do nothing;
                -- Ensure all the queues exist
                insert into :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues (queue_name)
                select distinct spec.queue_name
                from unnest(specs) spec
                where spec.queue_name is not null
                on conflict do nothing;
                -- Ensure any locked jobs have their key cleared - in the case of locked
                -- existing job create a new job instead as it must have already started
                -- executing (i.e., it's world state is out of date, and the fact add_job
                -- has been called again implies there's new information that needs to be
                -- acted upon).
                update :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs
                set
                    key = null,
                    attempts = jobs.max_attempts,
                    updated_at = now()
                from unnest(specs) spec
                where spec.job_key is not null
                and jobs.key = spec.job_key
                and is_available is not true;
                -- TODO: is there a risk that a conflict could occur depending on the
                -- isolation level?
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
                    -- always reset error/retry state
                    attempts = 0,
                    last_error = null,
                    updated_at = now()
                where jobs.locked_at is null
                returning *;
            end;
            $$;
        "#},
        // Drop and create new 'complete_jobs' function
        indoc! {r#"
            DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.complete_jobs;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.complete_jobs(job_ids bigint[]) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
                LANGUAGE sql
                AS $$
                delete from :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs
                    where id = any(job_ids)
                    and (
                        locked_at is null
                    or
                        locked_at < now() - interval '4 hours'
                    )
                    returning *;
            $$;
        "#},
        // Drop and create new 'force_unlock_workers' function
        indoc! {r#"
            DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.force_unlock_workers;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.force_unlock_workers(worker_ids text[]) RETURNS void
                LANGUAGE sql
                AS $$
                    update :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs
                    set locked_at = null, locked_by = null
                    where locked_by = any(worker_ids);
                    update :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues
                    set locked_at = null, locked_by = null
                    where locked_by = any(worker_ids);
            $$;
        "#},
        // Drop and create new 'permanently_fail_jobs' function
        indoc! {r#"
            DROP FUNCTION IF EXISTS :GRAPHILE_WORKER_SCHEMA.permanently_fail_jobs;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.permanently_fail_jobs(job_ids bigint[], error_message text DEFAULT NULL::text) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
            LANGUAGE sql
            AS $$
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
                RETURNING *;
            $$;
        "#},
        // Drop and create new 'remove_job' function
        indoc! {r#"
            DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.remove_job;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.remove_job(job_key text) RETURNS :GRAPHILE_WORKER_SCHEMA._private_jobs
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
                RETURN v_job;
            end;
            $$;
        "#},
        // Drop and create new 'reschedule_jobs' function
        indoc! {r#"
            DROP FUNCTION :GRAPHILE_WORKER_SCHEMA.reschedule_jobs;
        "#},
        indoc! {r#"
            CREATE FUNCTION :GRAPHILE_WORKER_SCHEMA.reschedule_jobs(job_ids bigint[], run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, priority integer DEFAULT NULL::integer, attempts integer DEFAULT NULL::integer, max_attempts integer DEFAULT NULL::integer) RETURNS SETOF :GRAPHILE_WORKER_SCHEMA._private_jobs
            LANGUAGE sql
            AS $$
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
                RETURNING *;
            $$;
        "#},
    ],
};

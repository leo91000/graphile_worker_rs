use indoc::indoc;

use super::GraphileWorkerMigration;

pub const M000019_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000019",
    is_breaking: false,
    stmts: &[
        indoc! {r#"
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
                on conflict do nothing;

                insert into :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues (queue_name)
                select distinct spec.queue_name
                from unnest(specs) spec
                where spec.queue_name is not null
                on conflict do nothing;

                update :GRAPHILE_WORKER_SCHEMA._private_jobs as jobs
                set
                    key = null,
                    attempts = jobs.max_attempts,
                    updated_at = now()
                from unnest(specs) spec
                where spec.job_key is not null
                and jobs.key = spec.job_key
                and is_available is not true;

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
        "#},
        indoc! {r#"
            CREATE OR REPLACE FUNCTION :GRAPHILE_WORKER_SCHEMA.add_job(identifier text, payload json DEFAULT NULL::json, queue_name text DEFAULT NULL::text, run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, max_attempts integer DEFAULT NULL::integer, job_key text DEFAULT NULL::text, priority integer DEFAULT NULL::integer, flags text[] DEFAULT NULL::text[], job_key_mode text DEFAULT 'replace'::text) RETURNS :GRAPHILE_WORKER_SCHEMA._private_jobs
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

                    if v_job.id is null then
                        raise exception 'Unable to schedule job for key "%"', coalesce(job_key, '<null>') using errcode = 'GWNOR';
                    end if;

                    return v_job;
                elsif job_key_mode = 'unsafe_dedupe' then
                    insert into :GRAPHILE_WORKER_SCHEMA._private_tasks as tasks (identifier)
                    values (identifier)
                    on conflict do nothing;

                    if queue_name is not null then
                        insert into :GRAPHILE_WORKER_SCHEMA._private_job_queues as job_queues (queue_name)
                        values (queue_name)
                        on conflict do nothing;
                    end if;

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
                    on job_queues.queue_name = add_job.queue_name
                    where tasks.identifier = add_job.identifier
                    on conflict (key)
                    do update set
                        revision = jobs.revision + 1,
                        updated_at = now()
                    returning *
                    into v_job;

                    if v_job.revision = 0 then
                        perform pg_notify('jobs:insert', '{"r":' || random()::text || ',"count":1}');
                    end if;

                    return v_job;
                else
                    raise exception 'Invalid job_key_mode value, expected ''replace'', ''preserve_run_at'' or ''unsafe_dedupe''.' using errcode = 'GWBKM';
                end if;
            end;
            $$;
        "#},
    ],
};

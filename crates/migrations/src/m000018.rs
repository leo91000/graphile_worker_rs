pub const M000018_MIGRATION: &[&str] = &[
    // Drop the trigger and function
    r#"
        DROP TRIGGER _900_after_insert ON :ARCHIMEDES_SCHEMA._private_jobs;
    "#,
    r#"
        DROP FUNCTION :ARCHIMEDES_SCHEMA.tg_jobs__after_insert;
    "#,
    // Create or replace 'add_job' function with full function body
    r#"
        CREATE OR REPLACE FUNCTION :ARCHIMEDES_SCHEMA.add_job(identifier text, payload json DEFAULT NULL::json, queue_name text DEFAULT NULL::text, run_at timestamp with time zone DEFAULT NULL::timestamp with time zone, max_attempts integer DEFAULT NULL::integer, job_key text DEFAULT NULL::text, priority integer DEFAULT NULL::integer, flags text[] DEFAULT NULL::text[], job_key_mode text DEFAULT 'replace'::text) RETURNS :ARCHIMEDES_SCHEMA._private_jobs
        LANGUAGE plpgsql
        AS $$
        declare
          v_job :ARCHIMEDES_SCHEMA._private_jobs;
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
            -- Ensure all the tasks exist
            insert into :ARCHIMEDES_SCHEMA._private_tasks as tasks (identifier)
            values (identifier)
            on conflict do nothing;
            -- Ensure all the queues exist
            if queue_name is not null then
              insert into :ARCHIMEDES_SCHEMA._private_job_queues as job_queues (queue_name)
              values (queue_name)
              on conflict do nothing;
            end if;
            -- Insert job, but if one already exists then do nothing
            insert into :ARCHIMEDES_SCHEMA._private_jobs as jobs (
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
            from :ARCHIMEDES_SCHEMA._private_tasks as tasks
            left join :ARCHIMEDES_SCHEMA._private_job_queues as job_queues
            on job_queues.queue_name = queue_name
            where tasks.identifier = identifier
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
    "#,
    // Create or replace 'add_jobs' function with full function body
    r#"
        CREATE OR REPLACE FUNCTION :ARCHIMEDES_SCHEMA.add_jobs(specs :ARCHIMEDES_SCHEMA.job_spec[], job_key_preserve_run_at boolean DEFAULT false) RETURNS SETOF :ARCHIMEDES_SCHEMA._private_jobs
        LANGUAGE plpgsql
        AS $$
        begin
          -- Ensure all the tasks exist
          insert into :ARCHIMEDES_SCHEMA._private_tasks as tasks (identifier)
          select distinct spec.identifier
          from unnest(specs) spec
          on conflict do nothing;
          -- Ensure all the queues exist
          insert into :ARCHIMEDES_SCHEMA._private_job_queues as job_queues (queue_name)
          select distinct spec.queue_name
          from unnest(specs) spec
          where spec.queue_name is not null
          on conflict do nothing;
          -- Ensure any locked jobs have their key cleared - in the case of locked
          -- existing job create a new job instead as it must have already started
          -- executing (i.e. it's world state is out of date, and the fact add_job
          -- has been called again implies there's new information that needs to be
          -- acted upon).
          update :ARCHIMEDES_SCHEMA._private_jobs as jobs
          set
            key = null,
            attempts = jobs.max_attempts,
            updated_at = now()
          from unnest(specs) spec
          where spec.job_key is not null
          and jobs.key = spec.job_key
          and is_available is not true;

          -- WARNING: this count is not 100% accurate; 'on conflict' clause will cause it to be an overestimate
          perform pg_notify('jobs:insert', '{"r":' || random()::text || ',"count":' || array_length(specs, 1)::text || '}');

          -- TODO: is there a risk that a conflict could occur depending on the
          -- isolation level?
          return query insert into :ARCHIMEDES_SCHEMA._private_jobs as jobs (
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
            inner join :ARCHIMEDES_SCHEMA._private_tasks as tasks
            on tasks.identifier = spec.identifier
            left join :ARCHIMEDES_SCHEMA._private_job_queues as job_queues
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
    "#,
    // Create or replace 'remove_job' function with full function body
    r#"
        CREATE OR REPLACE FUNCTION :ARCHIMEDES_SCHEMA.remove_job(job_key text) RETURNS :ARCHIMEDES_SCHEMA._private_jobs
        LANGUAGE plpgsql STRICT
        AS $$
        declare
          v_job :ARCHIMEDES_SCHEMA._private_jobs;
        begin
          -- Delete job if not locked
          DELETE FROM :ARCHIMEDES_SCHEMA._private_jobs AS jobs
          WHERE key = job_key
          AND (
            locked_at IS NULL
          OR
            locked_at < NOW() - interval '4 hours'
          )
          RETURNING * INTO v_job;
          IF NOT (v_job IS NULL) THEN
            perform pg_notify('jobs:insert', '{"r":' || random()::text || ',"count":-1}');
            RETURN v_job;
          END IF;
          -- Otherwise prevent job from retrying, and clear the key
          UPDATE :ARCHIMEDES_SCHEMA._private_jobs AS jobs
          SET
            key = NULL,
            attempts = jobs.max_attempts,
            updated_at = now()
          WHERE key = job_key
          RETURNING * INTO v_job;
          RETURN v_job;
        end;
        $$;
    "#,
];

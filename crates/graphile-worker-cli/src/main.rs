use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use graphile_worker::utils::escape_identifier;
use graphile_worker::worker_utils::{CleanupTask, RescheduleJobOptions};
use graphile_worker::{Database, DbJob, Job, JobKeyMode, JobSpec, WorkerUtils};
use graphile_worker_admin_ui::{AdminAuthConfig, AdminServerConfig};
use indoc::formatdoc;
use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

#[derive(Parser, Debug)]
#[command(
    name = "graphile-worker",
    about = "Manage Graphile Worker jobs in PostgreSQL",
    version
)]
struct Cli {
    /// PostgreSQL connection URL. Falls back to DATABASE_URL.
    #[arg(long, env = "DATABASE_URL", global = true)]
    database_url: Option<String>,

    /// Graphile Worker schema name. Falls back to GRAPHILE_WORKER_SCHEMA.
    #[arg(
        long,
        env = "GRAPHILE_WORKER_SCHEMA",
        default_value = "graphile_worker",
        global = true
    )]
    schema: String,

    /// Maximum PostgreSQL connections used by the CLI.
    #[arg(long, default_value_t = 5, global = true)]
    max_connections: u32,

    /// Print machine-readable JSON where supported.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Add a raw job to the queue.
    Add(AddArgs),

    /// List jobs, optionally filtered by state, task, or queue.
    List(ListArgs),

    /// Show one job by id.
    Show(ShowArgs),

    /// Mark jobs as completed.
    Complete(JobIdsArgs),

    /// Mark jobs as permanently failed.
    Fail(FailArgs),

    /// Reschedule jobs or update retry metadata.
    Reschedule(RescheduleArgs),

    /// Remove a job by job key.
    Remove(RemoveArgs),

    /// Run maintenance cleanup tasks.
    Cleanup(CleanupArgs),

    /// Force unlock jobs and queues locked by worker ids.
    ForceUnlock(ForceUnlockArgs),

    /// Run Graphile Worker migrations.
    Migrate,

    /// Print queue-wide job counts.
    Stats,

    /// List known queues and their lock state.
    Queues,

    /// List worker ids that currently hold locks.
    Workers,

    /// Serve the embedded Leptos admin UI and JSON management API.
    Admin(AdminArgs),
}

#[derive(Args, Debug)]
struct AddArgs {
    /// Task identifier.
    identifier: String,

    /// JSON payload. Defaults to {}.
    #[arg(long, conflicts_with = "payload_file")]
    payload: Option<String>,

    /// Read JSON payload from a file.
    #[arg(long, conflicts_with = "payload")]
    payload_file: Option<PathBuf>,

    /// Queue name.
    #[arg(long)]
    queue: Option<String>,

    /// RFC 3339 run time, or "now".
    #[arg(long, value_parser = parse_utc_datetime)]
    run_at: Option<DateTime<Utc>>,

    /// Maximum retry attempts.
    #[arg(long)]
    max_attempts: Option<i16>,

    /// Job key used for deduplication or replacement.
    #[arg(long)]
    key: Option<String>,

    /// Job key behavior.
    #[arg(long, value_enum, requires = "key")]
    job_key_mode: Option<JobKeyModeArg>,

    /// Job priority. Lower values run sooner.
    #[arg(long)]
    priority: Option<i16>,

    /// Job flags. Can be passed multiple times.
    #[arg(long = "flag")]
    flags: Vec<String>,
}

#[derive(Args, Debug)]
struct ListArgs {
    /// Filter by task identifier.
    #[arg(long)]
    identifier: Option<String>,

    /// Filter by queue name.
    #[arg(long)]
    queue: Option<String>,

    /// Filter by job state.
    #[arg(long, value_enum, default_value_t = JobState::All)]
    state: JobState,

    /// Maximum jobs to return.
    #[arg(long, default_value_t = 50)]
    limit: i64,

    /// Number of jobs to skip.
    #[arg(long, default_value_t = 0)]
    offset: i64,
}

#[derive(Args, Debug)]
struct ShowArgs {
    /// Job id.
    id: i64,
}

#[derive(Args, Debug)]
struct JobIdsArgs {
    /// Job ids.
    #[arg(required = true)]
    ids: Vec<i64>,
}

#[derive(Args, Debug)]
struct FailArgs {
    /// Job ids.
    #[arg(required = true)]
    ids: Vec<i64>,

    /// Failure reason.
    #[arg(short, long, default_value = "Manually marked as failed")]
    reason: String,
}

#[derive(Args, Debug)]
struct RescheduleArgs {
    /// Job ids.
    #[arg(required = true)]
    ids: Vec<i64>,

    /// Set run_at to now.
    #[arg(long, conflicts_with = "run_at")]
    now: bool,

    /// RFC 3339 run time.
    #[arg(long, value_parser = parse_utc_datetime)]
    run_at: Option<DateTime<Utc>>,

    /// New priority.
    #[arg(long)]
    priority: Option<i16>,

    /// New attempt count.
    #[arg(long)]
    attempts: Option<i16>,

    /// New maximum retry attempts.
    #[arg(long)]
    max_attempts: Option<i16>,
}

#[derive(Args, Debug)]
struct RemoveArgs {
    /// Job key to remove.
    key: String,
}

#[derive(Args, Debug)]
struct CleanupArgs {
    /// Cleanup tasks to run. Defaults to all tasks when omitted.
    #[arg(value_enum)]
    tasks: Vec<CleanupTaskArg>,
}

#[derive(Args, Debug)]
struct ForceUnlockArgs {
    /// Worker ids to unlock.
    #[arg(required = true)]
    worker_ids: Vec<String>,
}

#[derive(Args, Debug)]
struct AdminArgs {
    /// Address for the admin HTTP server.
    #[arg(long, default_value = "127.0.0.1:5678")]
    listen: SocketAddr,

    /// Authentication mode for the admin UI.
    #[arg(long, value_enum, default_value_t = AdminAuthModeArg::Basic)]
    auth: AdminAuthModeArg,

    /// HTTP Basic username.
    #[arg(long, default_value = "admin")]
    username: String,

    /// HTTP Basic password. Generated randomly when omitted.
    #[arg(long, env = "GRAPHILE_WORKER_ADMIN_PASSWORD")]
    password: Option<String>,

    /// Bearer token. Generated randomly when --auth bearer and omitted.
    #[arg(long, env = "GRAPHILE_WORKER_ADMIN_BEARER_TOKEN")]
    bearer_token: Option<String>,

    /// Header token. Generated randomly when --auth header and omitted.
    #[arg(long, env = "GRAPHILE_WORKER_ADMIN_HEADER_TOKEN")]
    header_token: Option<String>,

    /// Header name for --auth header.
    #[arg(long, default_value = "x-graphile-worker-admin-token")]
    header_name: String,

    /// Disable all mutating admin actions.
    #[arg(long)]
    read_only: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AdminAuthModeArg {
    Basic,
    Bearer,
    Header,
    None,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum JobState {
    All,
    Ready,
    Scheduled,
    Locked,
    Failed,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum JobKeyModeArg {
    Replace,
    PreserveRunAt,
    UnsafeDedupe,
}

impl From<JobKeyModeArg> for JobKeyMode {
    fn from(value: JobKeyModeArg) -> Self {
        match value {
            JobKeyModeArg::Replace => JobKeyMode::Replace,
            JobKeyModeArg::PreserveRunAt => JobKeyMode::PreserveRunAt,
            JobKeyModeArg::UnsafeDedupe => JobKeyMode::UnsafeDedupe,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
enum CleanupTaskArg {
    DeletePermanentlyFailedJobs,
    GcTaskIdentifiers,
    GcJobQueues,
}

impl CleanupTaskArg {
    fn all() -> Vec<Self> {
        vec![
            Self::DeletePermanentlyFailedJobs,
            Self::GcTaskIdentifiers,
            Self::GcJobQueues,
        ]
    }
}

impl From<CleanupTaskArg> for CleanupTask {
    fn from(value: CleanupTaskArg) -> Self {
        match value {
            CleanupTaskArg::DeletePermanentlyFailedJobs => CleanupTask::DeletePermenantlyFailedJobs,
            CleanupTaskArg::GcTaskIdentifiers => CleanupTask::GcTaskIdentifiers,
            CleanupTaskArg::GcJobQueues => CleanupTask::GcJobQueues,
        }
    }
}

#[derive(Debug, FromRow, Serialize)]
struct ListedJob {
    id: i64,
    task_identifier: String,
    queue_name: Option<String>,
    payload: Value,
    priority: i16,
    run_at: DateTime<Utc>,
    attempts: i16,
    max_attempts: i16,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    key: Option<String>,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    revision: i32,
    flags: Option<Value>,
    is_available: bool,
}

#[derive(Debug, FromRow, Serialize)]
struct JobStats {
    total: i64,
    ready: i64,
    scheduled: i64,
    locked: i64,
    failed: i64,
}

#[derive(Debug, FromRow, Serialize)]
struct QueueRow {
    id: i32,
    queue_name: String,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    job_count: i64,
    ready_count: i64,
}

#[derive(Debug, FromRow, Serialize)]
struct LockedWorkerRow {
    worker_id: String,
    locked_jobs: i64,
    locked_queues: i64,
}

#[derive(Debug, Serialize)]
struct DbJobOutput {
    id: i64,
    task_id: i32,
    task_identifier: Option<String>,
    job_queue_id: Option<i32>,
    payload: Value,
    priority: i16,
    run_at: DateTime<Utc>,
    attempts: i16,
    max_attempts: i16,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    key: Option<String>,
    revision: i32,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    flags: Option<Value>,
}

impl DbJobOutput {
    fn from_db_job(job: &DbJob) -> Self {
        Self {
            id: *job.id(),
            task_id: *job.task_id(),
            task_identifier: None,
            job_queue_id: *job.job_queue_id(),
            payload: job.payload().clone(),
            priority: *job.priority(),
            run_at: *job.run_at(),
            attempts: *job.attempts(),
            max_attempts: *job.max_attempts(),
            last_error: job.last_error().clone(),
            created_at: *job.created_at(),
            updated_at: *job.updated_at(),
            key: job.key().clone(),
            revision: *job.revision(),
            locked_at: *job.locked_at(),
            locked_by: job.locked_by().clone(),
            flags: job.flags().clone(),
        }
    }

    fn from_job(job: &Job) -> Self {
        Self {
            id: *job.id(),
            task_id: *job.task_id(),
            task_identifier: Some(job.task_identifier().clone()),
            job_queue_id: *job.job_queue_id(),
            payload: job.payload().clone(),
            priority: *job.priority(),
            run_at: *job.run_at(),
            attempts: *job.attempts(),
            max_attempts: *job.max_attempts(),
            last_error: job.last_error().clone(),
            created_at: *job.created_at(),
            updated_at: *job.updated_at(),
            key: job.key().clone(),
            revision: *job.revision(),
            locked_at: *job.locked_at(),
            locked_by: job.locked_by().clone(),
            flags: job.flags().clone(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let database_url = cli
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("missing database URL; pass --database-url or set DATABASE_URL"))?;

    let pool = PgPoolOptions::new()
        .max_connections(cli.max_connections)
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    let database: Database = pool.clone().into();
    let escaped_schema = escape_identifier(&database, &cli.schema)
        .await
        .with_context(|| format!("failed to escape schema name `{}`", cli.schema))?;
    let utils = WorkerUtils::new(database, escaped_schema.clone());

    run_command(&cli, &pool, &utils, &escaped_schema).await
}

async fn run_command(
    cli: &Cli,
    pool: &PgPool,
    utils: &WorkerUtils,
    escaped_schema: &str,
) -> Result<()> {
    match &cli.command {
        Command::Add(args) => {
            let payload = read_payload(args)?;
            let spec = JobSpec {
                queue_name: args.queue.clone(),
                run_at: args.run_at,
                max_attempts: args.max_attempts,
                job_key: args.key.clone(),
                job_key_mode: args.job_key_mode.map(Into::into),
                priority: args.priority,
                flags: (!args.flags.is_empty()).then(|| args.flags.clone()),
            };
            let job = utils
                .add_raw_job(&args.identifier, payload, spec)
                .await
                .context("failed to add job")?;
            if cli.json {
                print_json(&DbJobOutput::from_job(&job))?;
            } else {
                println!(
                    "Added job {} for task `{}`",
                    job.id(),
                    job.task_identifier()
                );
            }
        }
        Command::List(args) => {
            let jobs = list_jobs(pool, escaped_schema, args).await?;
            if cli.json {
                print_json(&jobs)?;
            } else {
                print_jobs_table(&jobs);
            }
        }
        Command::Show(args) => {
            let job = get_job(pool, escaped_schema, args.id).await?;
            if cli.json {
                print_json(&job)?;
            } else {
                print_job_details(&job)?;
            }
        }
        Command::Complete(args) => {
            let jobs = utils
                .complete_jobs(&args.ids)
                .await
                .context("failed to complete jobs")?;
            print_db_job_result(cli.json, "Completed", &jobs)?;
        }
        Command::Fail(args) => {
            let jobs = utils
                .permanently_fail_jobs(&args.ids, &args.reason)
                .await
                .context("failed to permanently fail jobs")?;
            print_db_job_result(cli.json, "Failed", &jobs)?;
        }
        Command::Reschedule(args) => {
            let run_at = if args.now {
                Some(Utc::now())
            } else {
                args.run_at
            };
            if run_at.is_none()
                && args.priority.is_none()
                && args.attempts.is_none()
                && args.max_attempts.is_none()
            {
                return Err(anyhow!(
                    "provide at least one of --now, --run-at, --priority, --attempts, or --max-attempts"
                ));
            }

            let jobs = utils
                .reschedule_jobs(
                    &args.ids,
                    RescheduleJobOptions {
                        run_at,
                        priority: args.priority,
                        attempts: args.attempts,
                        max_attempts: args.max_attempts,
                    },
                )
                .await
                .context("failed to reschedule jobs")?;
            print_db_job_result(cli.json, "Rescheduled", &jobs)?;
        }
        Command::Remove(args) => {
            utils
                .remove_job(&args.key)
                .await
                .with_context(|| format!("failed to remove job with key `{}`", args.key))?;
            if cli.json {
                print_json(&serde_json::json!({ "removed_key": args.key }))?;
            } else {
                println!("Removed job with key `{}`", args.key);
            }
        }
        Command::Cleanup(args) => {
            let requested_tasks = if args.tasks.is_empty() {
                CleanupTaskArg::all()
            } else {
                args.tasks.clone()
            };
            let tasks: Vec<CleanupTask> = requested_tasks.iter().copied().map(Into::into).collect();
            utils.cleanup(&tasks).await.context("failed to cleanup")?;
            if cli.json {
                print_json(&serde_json::json!({ "cleanup_tasks": requested_tasks }))?;
            } else {
                println!("Cleanup complete");
            }
        }
        Command::ForceUnlock(args) => {
            let worker_ids: Vec<&str> = args.worker_ids.iter().map(String::as_str).collect();
            utils
                .force_unlock_workers(&worker_ids)
                .await
                .context("failed to force unlock workers")?;
            if cli.json {
                print_json(&serde_json::json!({ "unlocked_workers": args.worker_ids }))?;
            } else {
                println!("Unlocked {} worker id(s)", args.worker_ids.len());
            }
        }
        Command::Migrate => {
            utils.migrate().await.context("failed to run migrations")?;
            if cli.json {
                print_json(&serde_json::json!({ "migrated": true }))?;
            } else {
                println!("Migrations complete");
            }
        }
        Command::Stats => {
            let stats = get_stats(pool, escaped_schema).await?;
            if cli.json {
                print_json(&stats)?;
            } else {
                println!("total\tready\tscheduled\tlocked\tfailed");
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    stats.total, stats.ready, stats.scheduled, stats.locked, stats.failed
                );
            }
        }
        Command::Queues => {
            let queues = list_queues(pool, escaped_schema).await?;
            if cli.json {
                print_json(&queues)?;
            } else {
                print_queues_table(&queues);
            }
        }
        Command::Workers => {
            let workers = list_locked_workers(pool, escaped_schema).await?;
            if cli.json {
                print_json(&workers)?;
            } else {
                print_workers_table(&workers);
            }
        }
        Command::Admin(args) => {
            let auth = build_admin_auth(args)?;
            print_admin_startup(args, &auth);
            let config = AdminServerConfig::builder(pool.clone(), utils.clone())
                .escaped_schema(escaped_schema)
                .schema(cli.schema.clone())
                .listen_addr(args.listen)
                .auth(auth)
                .read_only(args.read_only)
                .build()?;
            graphile_worker_admin_ui::serve(config)
                .await
                .context("admin UI server stopped with an error")?;
        }
    }

    Ok(())
}

fn build_admin_auth(args: &AdminArgs) -> Result<AdminAuthConfig> {
    match args.auth {
        AdminAuthModeArg::Basic => Ok(match &args.password {
            Some(password) => AdminAuthConfig::basic(&args.username, password),
            None => AdminAuthConfig::basic_with_random_password(&args.username),
        }),
        AdminAuthModeArg::Bearer => {
            let (token, generated) = match &args.bearer_token {
                Some(token) => (token.clone(), false),
                None => (graphile_worker_admin_ui::generate_secret(), true),
            };
            Ok(AdminAuthConfig::bearer(token, generated))
        }
        AdminAuthModeArg::Header => {
            let (token, generated) = match &args.header_token {
                Some(token) => (token.clone(), false),
                None => (graphile_worker_admin_ui::generate_secret(), true),
            };
            AdminAuthConfig::header(&args.header_name, token, generated)
                .context("invalid admin header auth configuration")
        }
        AdminAuthModeArg::None => {
            if !args.listen.ip().is_loopback() {
                return Err(anyhow!(
                    "--auth none is only allowed when --listen uses a loopback address"
                ));
            }
            Ok(AdminAuthConfig::None)
        }
    }
}

fn print_admin_startup(args: &AdminArgs, auth: &AdminAuthConfig) {
    println!("Graphile Worker admin UI: http://{}", args.listen);
    println!(
        "Read-only mode: {}",
        if args.read_only { "on" } else { "off" }
    );

    match auth {
        AdminAuthConfig::Basic {
            username,
            generated_password,
            ..
        } => {
            println!("Auth: HTTP Basic");
            println!("Username: {username}");
            if *generated_password {
                if let Some(password) = auth.secret_for_display() {
                    println!("Generated password: {password}");
                }
            } else {
                println!("Password: configured");
            }
        }
        AdminAuthConfig::Bearer {
            generated_token, ..
        } => {
            println!("Auth: bearer token");
            if *generated_token {
                if let Some(token) = auth.secret_for_display() {
                    println!("Generated bearer token: {token}");
                }
            } else {
                println!("Bearer token: configured");
            }
        }
        AdminAuthConfig::Header {
            header_name,
            generated_token,
            ..
        } => {
            println!("Auth: header token");
            println!("Header: {}", header_name.as_str());
            if *generated_token {
                if let Some(token) = auth.secret_for_display() {
                    println!("Generated header token: {token}");
                }
            } else {
                println!("Header token: configured");
            }
        }
        AdminAuthConfig::None => {
            println!("Auth: none (loopback only)");
        }
    }
}

fn read_payload(args: &AddArgs) -> Result<Value> {
    let payload = match (&args.payload, &args.payload_file) {
        (Some(payload), None) => payload.clone(),
        (None, Some(path)) => fs::read_to_string(path)
            .with_context(|| format!("failed to read payload file `{}`", path.display()))?,
        (None, None) => "{}".to_string(),
        (Some(_), Some(_)) => unreachable!("clap prevents payload and payload_file together"),
    };

    serde_json::from_str(&payload).context("payload must be valid JSON")
}

async fn list_jobs(pool: &PgPool, escaped_schema: &str, args: &ListArgs) -> Result<Vec<ListedJob>> {
    if args.limit < 0 {
        return Err(anyhow!("--limit must be greater than or equal to 0"));
    }
    if args.offset < 0 {
        return Err(anyhow!("--offset must be greater than or equal to 0"));
    }

    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
                jobs.priority,
                jobs.run_at,
                jobs.attempts,
                jobs.max_attempts,
                jobs.last_error,
                jobs.created_at,
                jobs.updated_at,
                jobs.key,
                jobs.locked_at,
                jobs.locked_by,
                jobs.revision,
                jobs.flags,
                jobs.is_available
            from {escaped_schema}._private_jobs as jobs
            inner join {escaped_schema}._private_tasks as tasks on tasks.id = jobs.task_id
            left join {escaped_schema}._private_job_queues as job_queues on job_queues.id = jobs.job_queue_id
            where true
        "#
    ));

    apply_job_filters(&mut query, args);
    query.push(" order by jobs.id asc limit ");
    query.push_bind(args.limit);
    query.push(" offset ");
    query.push_bind(args.offset);

    query
        .build_query_as()
        .fetch_all(pool)
        .await
        .context("failed to list jobs")
}

async fn get_job(pool: &PgPool, escaped_schema: &str, id: i64) -> Result<ListedJob> {
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
                jobs.priority,
                jobs.run_at,
                jobs.attempts,
                jobs.max_attempts,
                jobs.last_error,
                jobs.created_at,
                jobs.updated_at,
                jobs.key,
                jobs.locked_at,
                jobs.locked_by,
                jobs.revision,
                jobs.flags,
                jobs.is_available
            from {escaped_schema}._private_jobs as jobs
            inner join {escaped_schema}._private_tasks as tasks on tasks.id = jobs.task_id
            left join {escaped_schema}._private_job_queues as job_queues on job_queues.id = jobs.job_queue_id
            where jobs.id =
        "#
    ));
    query.push_bind(id);

    query
        .build_query_as()
        .fetch_one(pool)
        .await
        .with_context(|| format!("failed to get job {id}"))
}

fn apply_job_filters<'a>(query: &mut QueryBuilder<'a, Postgres>, args: &'a ListArgs) {
    if let Some(identifier) = &args.identifier {
        query.push(" and tasks.identifier = ");
        query.push_bind(identifier);
    }

    if let Some(queue) = &args.queue {
        query.push(" and job_queues.queue_name = ");
        query.push_bind(queue);
    }

    match args.state {
        JobState::All => {}
        JobState::Ready => {
            query.push(" and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at <= now()");
        }
        JobState::Scheduled => {
            query.push(" and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at > now()");
        }
        JobState::Locked => {
            query.push(" and jobs.locked_at is not null");
        }
        JobState::Failed => {
            query.push(" and jobs.locked_at is null and jobs.attempts >= jobs.max_attempts");
        }
    }
}

async fn get_stats(pool: &PgPool, escaped_schema: &str) -> Result<JobStats> {
    let sql = formatdoc!(
        r#"
            select
                count(*)::bigint as total,
                count(*) filter (
                    where locked_at is null
                    and attempts < max_attempts
                    and run_at <= now()
                )::bigint as ready,
                count(*) filter (
                    where locked_at is null
                    and attempts < max_attempts
                    and run_at > now()
                )::bigint as scheduled,
                count(*) filter (where locked_at is not null)::bigint as locked,
                count(*) filter (
                    where locked_at is null
                    and attempts >= max_attempts
                )::bigint as failed
            from {escaped_schema}._private_jobs
        "#
    );

    sqlx::query_as(&sql)
        .fetch_one(pool)
        .await
        .context("failed to get stats")
}

async fn list_queues(pool: &PgPool, escaped_schema: &str) -> Result<Vec<QueueRow>> {
    let sql = formatdoc!(
        r#"
            select
                job_queues.id,
                job_queues.queue_name,
                job_queues.locked_at,
                job_queues.locked_by,
                count(jobs.*)::bigint as job_count,
                count(jobs.*) filter (
                    where jobs.locked_at is null
                    and jobs.attempts < jobs.max_attempts
                    and jobs.run_at <= now()
                )::bigint as ready_count
            from {escaped_schema}._private_job_queues as job_queues
            left join {escaped_schema}._private_jobs as jobs on jobs.job_queue_id = job_queues.id
            group by job_queues.id, job_queues.queue_name, job_queues.locked_at, job_queues.locked_by
            order by job_queues.queue_name asc
        "#
    );

    sqlx::query_as(&sql)
        .fetch_all(pool)
        .await
        .context("failed to list queues")
}

async fn list_locked_workers(pool: &PgPool, escaped_schema: &str) -> Result<Vec<LockedWorkerRow>> {
    let sql = formatdoc!(
        r#"
            select
                worker_id,
                sum(locked_jobs)::bigint as locked_jobs,
                sum(locked_queues)::bigint as locked_queues
            from (
                select locked_by as worker_id, count(*)::bigint as locked_jobs, 0::bigint as locked_queues
                from {escaped_schema}._private_jobs
                where locked_by is not null
                group by locked_by
                union all
                select locked_by as worker_id, 0::bigint as locked_jobs, count(*)::bigint as locked_queues
                from {escaped_schema}._private_job_queues
                where locked_by is not null
                group by locked_by
            ) as locks
            group by worker_id
            order by worker_id asc
        "#
    );

    sqlx::query_as(&sql)
        .fetch_all(pool)
        .await
        .context("failed to list workers")
}

fn print_db_job_result(json: bool, action: &str, jobs: &[DbJob]) -> Result<()> {
    if json {
        let output: Vec<_> = jobs.iter().map(DbJobOutput::from_db_job).collect();
        print_json(&output)?;
        return Ok(());
    }

    let ids = jobs
        .iter()
        .map(|job| job.id().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    println!("{action} {} job(s): {ids}", jobs.len());
    Ok(())
}

fn print_jobs_table(jobs: &[ListedJob]) {
    println!("id\ttask\tqueue\tstate\trun_at\tattempts\tpriority\tkey\tlocked_by");
    for job in jobs {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}/{}\t{}\t{}\t{}",
            job.id,
            job.task_identifier,
            job.queue_name.as_deref().unwrap_or("-"),
            state_for(job),
            job.run_at.to_rfc3339(),
            job.attempts,
            job.max_attempts,
            job.priority,
            job.key.as_deref().unwrap_or("-"),
            job.locked_by.as_deref().unwrap_or("-")
        );
    }
}

fn print_job_details(job: &ListedJob) -> Result<()> {
    println!("id: {}", job.id);
    println!("task: {}", job.task_identifier);
    println!("queue: {}", job.queue_name.as_deref().unwrap_or("-"));
    println!("state: {}", state_for(job));
    println!("run_at: {}", job.run_at.to_rfc3339());
    println!("attempts: {}/{}", job.attempts, job.max_attempts);
    println!("priority: {}", job.priority);
    println!("key: {}", job.key.as_deref().unwrap_or("-"));
    println!("locked_by: {}", job.locked_by.as_deref().unwrap_or("-"));
    println!("last_error: {}", job.last_error.as_deref().unwrap_or("-"));
    println!("payload: {}", serde_json::to_string_pretty(&job.payload)?);
    Ok(())
}

fn print_queues_table(queues: &[QueueRow]) {
    println!("id\tqueue\tjobs\tready\tlocked_by\tlocked_at");
    for queue in queues {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            queue.id,
            queue.queue_name,
            queue.job_count,
            queue.ready_count,
            queue.locked_by.as_deref().unwrap_or("-"),
            queue
                .locked_at
                .map(|locked_at| locked_at.to_rfc3339())
                .unwrap_or_else(|| "-".to_string())
        );
    }
}

fn print_workers_table(workers: &[LockedWorkerRow]) {
    println!("worker_id\tlocked_jobs\tlocked_queues");
    for worker in workers {
        println!(
            "{}\t{}\t{}",
            worker.worker_id, worker.locked_jobs, worker.locked_queues
        );
    }
}

fn state_for(job: &ListedJob) -> &'static str {
    if job.locked_at.is_some() {
        return "locked";
    }
    if job.attempts >= job.max_attempts {
        return "failed";
    }
    if job.run_at > Utc::now() {
        return "scheduled";
    }
    "ready"
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_utc_datetime(value: &str) -> std::result::Result<DateTime<Utc>, String> {
    if value.eq_ignore_ascii_case("now") {
        return Ok(Utc::now());
    }

    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_datetime_as_utc() {
        let parsed =
            parse_utc_datetime("2026-01-02T03:04:05+02:00").expect("datetime should parse");

        assert_eq!(parsed.to_rfc3339(), "2026-01-02T01:04:05+00:00");
    }

    #[test]
    fn parses_now_datetime() {
        let before = Utc::now();
        let parsed = parse_utc_datetime("now").expect("now should parse");
        let after = Utc::now();

        assert!(parsed >= before);
        assert!(parsed <= after);
    }

    #[test]
    fn maps_cleanup_task_names() {
        let tasks: Vec<CleanupTask> = CleanupTaskArg::all().into_iter().map(Into::into).collect();

        assert_eq!(tasks.len(), 3);
    }
}

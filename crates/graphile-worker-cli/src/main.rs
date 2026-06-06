use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use graphile_worker::recovery::WorkerRecoveryConfig;
use graphile_worker::worker_utils::{CleanupTask, RescheduleJobOptions};
use graphile_worker::{
    escape_identifier, Database, DbJob, Job, JobKeyMode, JobSpec, SweepStaleWorkersOptions,
    WorkerUtils,
};
use graphile_worker_admin_api::queries::{self as admin_queries, ListJobsQueryOptions};
use graphile_worker_admin_api::{
    DbJobOutput, JobState as AdminJobState, ListJobsParams, ListedJob, LockedWorkerRow, QueueRow,
};
use graphile_worker_admin_ui::{AdminAuthConfig, AdminServerConfig};
use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

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

    /// Recover jobs from inactive workers and orphan locks.
    SweepStaleWorkers(SweepStaleWorkersArgs),

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
    #[arg(long, value_enum, default_value_t = CliJobState::All)]
    state: CliJobState,

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
struct SweepStaleWorkersArgs {
    /// Time since last heartbeat before a worker is deemed inactive (e.g. 5m, 300s).
    #[arg(long, value_parser = parse_duration)]
    sweep_threshold: Option<Duration>,

    /// Delay before recovered jobs are eligible to run again (e.g. 30s).
    #[arg(long, value_parser = parse_duration)]
    recovery_delay: Option<Duration>,

    /// List stale workers without recovering jobs.
    #[arg(long)]
    dry_run: bool,
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
enum CliJobState {
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

fn db_job_output_from_db_job(job: &DbJob) -> DbJobOutput {
    DbJobOutput {
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

fn db_job_output_from_job(job: &Job) -> DbJobOutput {
    DbJobOutput {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let database_url = cli
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("missing database URL; pass --database-url or set DATABASE_URL"))?;
    let escaped_schema = escape_identifier(&cli.schema);

    let pool = PgPoolOptions::new()
        .max_connections(cli.max_connections)
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    let database: Database = pool.clone().into();
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
                print_json(&db_job_output_from_job(&job))?;
            } else {
                println!(
                    "Added job {} for task `{}`",
                    job.id(),
                    job.task_identifier()
                );
            }
        }
        Command::List(args) => {
            let params = list_jobs_params(args)?;
            let jobs = admin_queries::list_jobs(
                pool,
                escaped_schema,
                &params,
                ListJobsQueryOptions::default(),
            )
            .await
            .context("failed to list jobs")?;
            if cli.json {
                print_json(&jobs)?;
            } else {
                print_jobs_table(&jobs);
            }
        }
        Command::Show(args) => {
            let job = admin_queries::get_job(pool, escaped_schema, args.id)
                .await
                .with_context(|| format!("failed to get job {}", args.id))?;
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
            let stats = admin_queries::get_stats(pool, escaped_schema)
                .await
                .context("failed to get stats")?;
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
            let queues = admin_queries::list_queues(pool, escaped_schema)
                .await
                .context("failed to list queues")?;
            if cli.json {
                print_json(&queues)?;
            } else {
                print_queues_table(&queues);
            }
        }
        Command::Workers => {
            let workers = admin_queries::list_locked_workers(pool, escaped_schema)
                .await
                .context("failed to list workers")?;
            if cli.json {
                print_json(&workers)?;
            } else {
                print_workers_table(&workers);
            }
        }
        Command::SweepStaleWorkers(args) => {
            let defaults = WorkerRecoveryConfig::default();
            let result = utils
                .sweep_stale_workers(SweepStaleWorkersOptions {
                    sweep_threshold: args.sweep_threshold.or(Some(defaults.sweep_threshold)),
                    recovery_delay: args.recovery_delay.or(Some(defaults.recovery_delay)),
                    dry_run: args.dry_run,
                    ..Default::default()
                })
                .await
                .context("failed to sweep stale workers")?;
            if cli.json {
                print_json(&result)?;
            } else if args.dry_run {
                if result.worker_ids.is_empty() {
                    println!("No stale workers found");
                } else {
                    println!(
                        "Would recover {} worker(s): {}",
                        result.worker_ids.len(),
                        result.worker_ids.join(", ")
                    );
                }
            } else if result.worker_ids.is_empty() {
                println!("No stale workers found");
            } else {
                println!(
                    "Recovered {} job(s) from {} worker(s): {}",
                    result.recovered_count,
                    result.worker_ids.len(),
                    result.worker_ids.join(", ")
                );
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

fn list_jobs_params(args: &ListArgs) -> Result<ListJobsParams> {
    if args.limit < 0 {
        return Err(anyhow!("--limit must be greater than or equal to 0"));
    }
    if args.offset < 0 {
        return Err(anyhow!("--offset must be greater than or equal to 0"));
    }

    Ok(ListJobsParams {
        state: match args.state {
            CliJobState::All => AdminJobState::All,
            CliJobState::Ready => AdminJobState::Ready,
            CliJobState::Scheduled => AdminJobState::Scheduled,
            CliJobState::Locked => AdminJobState::Locked,
            CliJobState::Failed => AdminJobState::Failed,
        },
        identifier: args.identifier.clone(),
        queue: args.queue.clone(),
        search: None,
        limit: args.limit,
        offset: args.offset,
    })
}

fn print_db_job_result(json: bool, action: &str, jobs: &[DbJob]) -> Result<()> {
    if json {
        let output: Vec<_> = jobs.iter().map(db_job_output_from_db_job).collect();
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

fn parse_duration(value: &str) -> std::result::Result<Duration, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("duration must not be empty".to_string());
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }

    let (number, unit) = value
        .find(|character: char| !character.is_ascii_digit())
        .map(|index| value.split_at(index))
        .ok_or_else(|| format!("invalid duration `{value}`"))?;

    let amount: u64 = number
        .parse()
        .map_err(|_| format!("invalid duration `{value}`"))?;

    let seconds = match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => amount,
        "m" | "min" | "mins" | "minute" | "minutes" => amount * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => amount * 60 * 60,
        unit => return Err(format!("unsupported duration unit `{unit}`")),
    };

    Ok(Duration::from_secs(seconds))
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

    #[test]
    fn parses_duration_units() {
        assert_eq!(
            parse_duration("300").expect("seconds"),
            Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration("5m").expect("minutes"),
            Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration("2h").expect("hours"),
            Duration::from_secs(7200)
        );
    }
}

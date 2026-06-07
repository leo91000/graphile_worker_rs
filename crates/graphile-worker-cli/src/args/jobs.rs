use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use graphile_worker::JobKeyMode;

use crate::parsers::parse_utc_datetime;

#[derive(Args, Debug)]
pub(crate) struct AddArgs {
    /// Task identifier.
    pub(crate) identifier: String,

    /// JSON payload. Defaults to {}.
    #[arg(long, conflicts_with = "payload_file")]
    pub(crate) payload: Option<String>,

    /// Read JSON payload from a file.
    #[arg(long, conflicts_with = "payload")]
    pub(crate) payload_file: Option<PathBuf>,

    /// Queue name.
    #[arg(long)]
    pub(crate) queue: Option<String>,

    /// RFC 3339 run time, or "now".
    #[arg(long, value_parser = parse_utc_datetime)]
    pub(crate) run_at: Option<DateTime<Utc>>,

    /// Maximum retry attempts.
    #[arg(long)]
    pub(crate) max_attempts: Option<i16>,

    /// Job key used for deduplication or replacement.
    #[arg(long)]
    pub(crate) key: Option<String>,

    /// Job key behavior.
    #[arg(long, value_enum, requires = "key")]
    pub(crate) job_key_mode: Option<JobKeyModeArg>,

    /// Job priority. Lower values run sooner.
    #[arg(long)]
    pub(crate) priority: Option<i16>,

    /// Job flags. Can be passed multiple times.
    #[arg(long = "flag")]
    pub(crate) flags: Vec<String>,
}

#[derive(Args, Debug)]
pub(crate) struct ListArgs {
    /// Filter by task identifier.
    #[arg(long)]
    pub(crate) identifier: Option<String>,

    /// Filter by queue name.
    #[arg(long)]
    pub(crate) queue: Option<String>,

    /// Filter by job state.
    #[arg(long, value_enum, default_value_t = CliJobState::All)]
    pub(crate) state: CliJobState,

    /// Maximum jobs to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: i64,

    /// Number of jobs to skip.
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: i64,
}

#[derive(Args, Debug)]
pub(crate) struct ShowArgs {
    /// Job id.
    pub(crate) id: i64,
}

#[derive(Args, Debug)]
pub(crate) struct JobIdsArgs {
    /// Job ids.
    #[arg(required = true)]
    pub(crate) ids: Vec<i64>,
}

#[derive(Args, Debug)]
pub(crate) struct FailArgs {
    /// Job ids.
    #[arg(required = true)]
    pub(crate) ids: Vec<i64>,

    /// Failure reason.
    #[arg(short, long, default_value = "Manually marked as failed")]
    pub(crate) reason: String,
}

#[derive(Args, Debug)]
pub(crate) struct RescheduleArgs {
    /// Job ids.
    #[arg(required = true)]
    pub(crate) ids: Vec<i64>,

    /// Set run_at to now.
    #[arg(long, conflicts_with = "run_at")]
    pub(crate) now: bool,

    /// RFC 3339 run time.
    #[arg(long, value_parser = parse_utc_datetime)]
    pub(crate) run_at: Option<DateTime<Utc>>,

    /// New priority.
    #[arg(long)]
    pub(crate) priority: Option<i16>,

    /// New attempt count.
    #[arg(long)]
    pub(crate) attempts: Option<i16>,

    /// New maximum retry attempts.
    #[arg(long)]
    pub(crate) max_attempts: Option<i16>,
}

#[derive(Args, Debug)]
pub(crate) struct RemoveArgs {
    /// Job key to remove.
    pub(crate) key: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum CliJobState {
    All,
    Ready,
    Scheduled,
    Locked,
    Failed,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum JobKeyModeArg {
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

use graphile_worker_crontab_types::Crontab;
pub use nom::error::ErrorKind;
use nom_crontab::nom_crontab;
use thiserror::Error;

mod nom_crontab;
mod nom_crontab_opts;
mod nom_crontab_payload;
mod nom_crontab_timer;
mod nom_task_identifier;

#[derive(Error, Debug)]
#[error("An error occured while parsing crontab : \n{msg}")]
pub struct CrontabParseError {
    pub msg: String,
    pub input: String,
    pub error_kind: ErrorKind,
}

impl<'a> From<nom::Err<nom::error::Error<&'a str>>> for CrontabParseError {
    fn from(e: nom::Err<nom::error::Error<&'a str>>) -> Self {
        let msg = format!("{e:?}");
        let (input, error_kind) = match e {
            // Should not happen (only for streams)
            nom::Err::Incomplete(_) => (String::from(""), ErrorKind::Fail),
            nom::Err::Error(e) | nom::Err::Failure(e) => (e.to_string(), e.code),
        };

        CrontabParseError {
            msg,
            input,
            error_kind,
        }
    }
}

/// Parse a crontab definition into a Vec of crontab
///
/// Tasks are by default read from a crontab file next to the tasks folder (but this is configurable in library mode).
/// Please note that our syntax is not 100% compatible with cron's, and our task payload differs.
/// We only handle timestamps in UTC. The following diagram details the parts of a Graphile Worker crontab schedule:
///
/// ```crontab
/// ┌───────────── UTC minute (0 - 59)
/// │ ┌───────────── UTC hour (0 - 23)
/// │ │ ┌───────────── UTC day of the month (1 - 31)
/// │ │ │ ┌───────────── UTC month (1 - 12)
/// │ │ │ │ ┌───────────── UTC day of the week (0 - 6) (Sunday to Saturday)
/// │ │ │ │ │ ┌───────────── task (identifier) to schedule
/// │ │ │ │ │ │    ┌────────── optional scheduling options
/// │ │ │ │ │ │    │     ┌────── optional payload to merge
/// │ │ │ │ │ │    │     │
/// │ │ │ │ │ │    │     │
/// * * * * * task ?opts {payload}
/// ```
///
/// Comment lines start with a `#`.
///
/// For the first 5 fields we support an explicit numeric value, `*` to represent
/// all valid values, `*/n` (where `n` is a positive integer) to represent all valid
/// values divisible by `n`, range syntax such as `1-5`, and any combination of
/// these separated by commas.
///
/// The task identifier should match the following regexp
/// `/^[_a-zA-Z][_a-zA-Z0-9:_-]*$/` (namely it should start with an alphabetic
/// character and it should only contain alphanumeric characters, colon, underscore
/// and hyphen). It should be the name of one of your Graphile Worker tasks.
///
/// The `opts` must always be prefixed with a `?` if provided and details
/// configuration for the task such as what should be done in the event that the
/// previous event was not scheduled (e.g. because the Worker wasn't running).
/// Options are specified using HTTP query string syntax (with `&` separator).
///
/// Currently we support the following `opts`:
///
/// - `id=UID` where UID is a unique alphanumeric case-sensitive identifier starting
///   with a letter - specify an identifier for this crontab entry; by default this
///   will use the task identifier, but if you want more than one schedule for the
///   same task (e.g. with different payload, or different times) then you will need
///   to supply a unique identifier explicitly.
/// - `fill=t` where `t` is a "time phrase" (see below) - backfill any entries from
///   the last time period `t`, for example if the worker was not running when they
///   were due to be executed (by default, no backfilling).
/// - `max=n` where `n` is a small positive integer - override the `max_attempts` of
///   the job.
/// - `queue=name` where `name` is an alphanumeric queue name - add the job to a
///   named queue so it executes serially.
/// - `priority=n` where `n` is a relatively small integer - override the priority
///   of the job.
///
/// **NOTE**: changing the identifier (e.g. via `id`) can result in duplicate
/// executions, so we recommend that you explicitly set it and never change it.
///
/// **NOTE**: using `fill` will not backfill new tasks, only tasks that were
/// previously known.
///
/// **NOTE**: the higher you set the `fill` parameter, the longer the worker startup
/// time will be; when used you should set it to be slightly larger than the longest
/// period of downtime you expect for your worker.
///
/// Time phrases are comprised of a sequence of number-letter combinations, where
/// the number represents a quantity and the letter represents a time period, e.g.
/// `5d` for `five days`, or `3h` for `three hours`; e.g. `4w3d2h1m` represents
/// `4 weeks, 3 days, 2 hours and 1 minute` (i.e. a period of 44761 minutes). The
/// following time periods are supported:
///
/// - `s` - one second (1000 milliseconds)
/// - `m` - one minute (60 seconds)
/// - `h` - one hour (60 minutes)
/// - `d` - one day (24 hours)
/// - `w` - one week (7 days)
///
/// The `payload` is a JSON5 object; it must start with a `{`, must not contain
/// newlines or carriage returns (`\n` or `\r`), and must not contain trailing
/// whitespace. It will be merged into the default crontab payload properties.
///
/// Each crontab job will have a JSON object payload containing the key `_cron` with
/// the value being an object with the following entries:
///
/// - `ts` - ISO8601 timestamp representing when this job was due to execute
/// - `backfilled` - true if the task was "backfilled" (i.e. it wasn't scheduled on time), false otherwise
pub fn parse_crontab(crontab: &str) -> Result<Vec<Crontab>, CrontabParseError> {
    let (_, result) = nom_crontab(crontab)?;
    Ok(result)
}

use nom::{bytes::complete::take_while1, character::complete, sequence::preceded, IResult, Parser};
use serde::Deserialize;

use graphile_worker_crontab_types::JobKeyMode;

/// Intermediate mirror of CrontabOptions.
///
/// This is used to parse the query string and allows `fill` to borrow from the
/// original input until it is parsed into a CrontabFill.
#[derive(Deserialize)]
pub(super) struct QueryOption<'a> {
    pub(super) id: Option<String>,
    pub(super) fill: Option<&'a str>,
    pub(super) max: Option<u16>,
    pub(super) queue: Option<String>,
    pub(super) priority: Option<i16>,
    pub(super) job_key: Option<String>,
    pub(super) job_key_mode: Option<JobKeyMode>,
}

pub(super) fn crontab_query(input: &str) -> IResult<&str, QueryOption<'_>> {
    let (input, qs) = preceded(
        complete::char('?'),
        take_while1(|c: char| !c.is_whitespace()),
    )
    .parse(input)?;

    Ok((
        input,
        serde_qs::from_str(qs).map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        })?,
    ))
}

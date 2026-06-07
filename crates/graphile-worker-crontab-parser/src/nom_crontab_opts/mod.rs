mod fill;
mod query;

use nom::{IResult, Parser};

use graphile_worker_crontab_types::CrontabOptions;

use self::fill::crontab_fill;
use self::query::crontab_query;

pub(crate) fn nom_crontab_opts(input: &str) -> IResult<&str, CrontabOptions> {
    let (input, query) = crontab_query.parse(input)?;

    let fill = query
        .fill
        .map(|v| crontab_fill(v))
        .transpose()?
        .map(|(_, v)| v);

    Ok((
        input,
        CrontabOptions {
            id: query.id,
            fill,
            max: query.max,
            queue: query.queue,
            priority: query.priority,
            job_key: query.job_key,
            job_key_mode: query.job_key_mode,
        },
    ))
}

#[cfg(test)]
mod tests;

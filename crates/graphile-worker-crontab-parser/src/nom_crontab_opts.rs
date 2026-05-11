use nom::{
    bytes::complete::take_while1,
    character::complete,
    combinator::{eof, opt},
    sequence::{preceded, terminated},
    IResult, Parser,
};
use serde::Deserialize;

use graphile_worker_crontab_types::{CrontabFill, CrontabOptions, JobKeyMode};

/// Intermediate miror of CrontabOptions
/// This is used to parse the query string
/// Specifically, this is needed to parse fill without using owned string
#[derive(Deserialize)]
struct QueryOption<'a> {
    id: Option<String>,
    fill: Option<&'a str>,
    max: Option<u16>,
    queue: Option<String>,
    priority: Option<i16>,
    job_key: Option<String>,
    job_key_mode: Option<JobKeyMode>,
}

fn crontab_query(input: &str) -> IResult<&str, QueryOption<'_>> {
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

fn crontab_fill(input: &str) -> IResult<&str, CrontabFill> {
    let (input, (w, d, h, m, s)) = (
        opt(terminated(complete::u32, complete::char('w'))),
        opt(terminated(complete::u32, complete::char('d'))),
        opt(terminated(complete::u32, complete::char('h'))),
        opt(terminated(complete::u32, complete::char('m'))),
        opt(terminated(complete::u32, complete::char('s'))),
    )
        .parse(input)?;

    eof.parse(input)?;

    Ok((
        input,
        CrontabFill {
            s: s.unwrap_or(0),
            m: m.unwrap_or(0),
            h: h.unwrap_or(0),
            d: d.unwrap_or(0),
            w: w.unwrap_or(0),
        },
    ))
}

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
mod tests {
    use nom::Parser;

    use super::*;

    #[test]
    fn test_valid_query() {
        let input = "?fill=4w3d2h1m&priority=-4 foo";
        assert_eq!(
            Ok((
                " foo",
                CrontabOptions {
                    fill: Some(CrontabFill {
                        s: 0,
                        m: 1,
                        h: 2,
                        d: 3,
                        w: 4
                    }),
                    priority: Some(-4),
                    ..Default::default()
                }
            )),
            nom_crontab_opts.parse(input)
        );

        let input = "?id=1234dfsd&max=4 bar";
        assert_eq!(
            Ok((
                " bar",
                CrontabOptions {
                    id: Some(String::from("1234dfsd")),
                    max: Some(4),
                    ..Default::default()
                }
            )),
            nom_crontab_opts.parse(input)
        );
    }

    #[test]
    fn test_query_not_preceded_by_question_mark() {
        let input = "fill=4w3d2h1m&priority=-4 foo";

        assert!(nom_crontab_opts.parse(input).is_err());
    }

    #[test]
    fn test_query_with_invalid_fill() {
        let input = "?fill=4w3d2h1m_bruh&priority=-4 foo";
        assert!(nom_crontab_opts.parse(input).is_err());

        let input = "?fill=4w_3d2h1m&priority=-4 foo";
        assert!(nom_crontab_opts.parse(input).is_err());
    }

    #[test]
    fn parse_job_key() {
        let input = "?job_key=foo";
        assert_eq!(
            Ok((
                "",
                CrontabOptions {
                    job_key: Some(String::from("foo")),
                    ..Default::default()
                }
            )),
            nom_crontab_opts.parse(input)
        );
    }

    #[test]
    fn parse_job_key_mode_replace() {
        let input = "?job_key_mode=replace";
        assert_eq!(
            Ok((
                "",
                CrontabOptions {
                    job_key_mode: Some(JobKeyMode::Replace),
                    ..Default::default()
                }
            )),
            nom_crontab_opts.parse(input)
        );
    }

    #[test]
    fn parse_job_key_mode_preserve_run_at() {
        let input = "?job_key_mode=preserve_run_at";
        assert_eq!(
            Ok((
                "",
                CrontabOptions {
                    job_key_mode: Some(JobKeyMode::PreserveRunAt),
                    ..Default::default()
                }
            )),
            nom_crontab_opts.parse(input)
        );
    }
}

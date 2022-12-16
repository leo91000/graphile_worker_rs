use nom::{
    bytes::complete::take_while1,
    character::complete,
    combinator::{eof, opt},
    sequence::{preceded, terminated, tuple},
    IResult,
};
use serde::Deserialize;

use crontab_types::{CrontabFill, CrontabOptions};

#[derive(Deserialize)]
struct QueryOption<'a> {
    id: Option<String>,
    fill: Option<&'a str>,
    max: Option<u16>,
    queue: Option<String>,
    priority: Option<i16>,
}

fn crontab_query(input: &str) -> IResult<&str, QueryOption<'_>> {
    let (input, qs) = preceded(
        complete::char('?'),
        take_while1(|c: char| !c.is_whitespace()),
    )(input)?;

    Ok((
        input,
        serde_qs::from_str(qs).map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        })?,
    ))
}

fn crontab_fill(input: &str) -> IResult<&str, CrontabFill> {
    let (input, (w, d, h, m, s)) = tuple((
        opt(terminated(complete::u32, complete::char('w'))),
        opt(terminated(complete::u32, complete::char('d'))),
        opt(terminated(complete::u32, complete::char('h'))),
        opt(terminated(complete::u32, complete::char('m'))),
        opt(terminated(complete::u32, complete::char('s'))),
    ))(input)?;

    eof(input)?;

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
    let (input, query) = crontab_query(input)?;

    let fill = match query.fill {
        Some(v) => Some(crontab_fill(v)?.1),
        None => None,
    };

    Ok((
        input,
        CrontabOptions {
            id: query.id,
            fill,
            max: query.max,
            queue: query.queue,
            priority: query.priority,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_query() {
        let input = "?fill=4w3d2h1m&priority=-4 foo";
        assert_eq!(
            Ok((
                " foo",
                CrontabOptions {
                    id: None,
                    fill: Some(CrontabFill {
                        s: 0,
                        m: 1,
                        h: 2,
                        d: 3,
                        w: 4
                    }),
                    max: None,
                    queue: None,
                    priority: Some(-4),
                }
            )),
            nom_crontab_opts(input)
        );

        let input = "?id=1234dfsd&max=4 bar";
        assert_eq!(
            Ok((
                " bar",
                CrontabOptions {
                    id: Some(String::from("1234dfsd")),
                    fill: None,
                    max: Some(4),
                    queue: None,
                    priority: None,
                }
            )),
            nom_crontab_opts(input)
        );
    }

    #[test]
    fn test_query_not_preceded_by_question_mark() {
        let input = "fill=4w3d2h1m&priority=-4 foo";

        assert!(nom_crontab_opts(input).is_err());
    }

    #[test]
    fn test_query_with_invalid_fill() {
        let input = "?fill=4w3d2h1m_bruh&priority=-4 foo";
        assert!(nom_crontab_opts(input).is_err());

        let input = "?fill=4w_3d2h1m&priority=-4 foo";
        assert!(nom_crontab_opts(input).is_err());
    }
}

use nom::{
    character::complete,
    combinator::{eof, opt},
    sequence::terminated,
    IResult, Parser,
};

use graphile_worker_crontab_types::CrontabFill;

pub(super) fn crontab_fill(input: &str) -> IResult<&str, CrontabFill> {
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

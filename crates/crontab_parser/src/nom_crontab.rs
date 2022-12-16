use nom::{
    bytes::complete::take_while,
    character::complete::{self, line_ending, multispace0, space0, space1},
    combinator::opt,
    multi::separated_list0,
    sequence::{delimited, preceded},
    IResult,
};

use crate::{
    nom_crontab_opts::nom_crontab_opts, nom_crontab_payload::nom_crontab_payload,
    nom_crontab_timer::nom_crontab_timer, nom_task_identifier::nom_task_identifier, Crontab,
};

fn crontab_line(input: &str) -> IResult<&str, Option<Crontab>> {
    let (input, timer) = preceded(space0, nom_crontab_timer)(input)?;

    let (input, task_identifier) = preceded(space1, nom_task_identifier)(input)?;
    let (input, options) = opt(preceded(space1, nom_crontab_opts))(input)?;
    let (input, payload) = opt(preceded(space1, nom_crontab_payload))(input)?;

    Ok((
        input,
        Some(Crontab {
            timer,
            task_identifier,
            options: options.unwrap_or_default(),
            payload,
        }),
    ))
}

fn crontab_comment(input: &str) -> IResult<&str, Option<Crontab>> {
    let (input, _) = preceded(
        preceded(space0, complete::char('#')),
        take_while(|c: char| c != '\n' && c != '\r'),
    )(input)?;

    Ok((input, None))
}

pub(crate) fn nom_crontab(input: &str) -> IResult<&str, Vec<Crontab>> {
    let (input, crontabs) = delimited(
        multispace0,
        separated_list0(
            line_ending,
            nom::branch::alt((crontab_comment, crontab_line)),
        ),
        multispace0,
    )(input)?;

    Ok((input, crontabs.into_iter().flatten().collect()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crontab_types::{CrontabFill, CrontabOptions, CrontabTimer, CrontabValue};

    use super::*;

    #[test]
    fn valid_crontab() {
        let input = r#"
            # ┌───────────── UTC minute (0 - 59)
            # │ ┌───────────── UTC hour (0 - 23)
            # │ │ ┌───────────── UTC day of the month (1 - 31)
            # │ │ │ ┌───────────── UTC month (1 - 12)
            # │ │ │ │ ┌───────────── UTC day of the week (0 - 6) (Sunday to Saturday)
            # │ │ │ │ │ ┌───────────── task (identifier) to schedule
            # │ │ │ │ │ │    ┌────────── optional scheduling options
            # │ │ │ │ │ │    │     ┌────── optional payload to merge
            # │ │ │ │ │ │    │     │
            # │ │ │ │ │ │    │     │
            # * * * * * task ?opts {payload}
            30 4 * * 1 send_weekly_email
            30 4,10-15 * * 1 send_weekly_email ?fill=2d&max=10 {onboarding:false}
            0 */4 * * * rollup
        "#;

        assert_eq!(
            Ok((
                "",
                vec![
                    Crontab {
                        timer: CrontabTimer {
                            minutes: vec![CrontabValue::Number(30)],
                            hours: vec![CrontabValue::Number(4)],
                            days: vec![CrontabValue::Any],
                            months: vec![CrontabValue::Any],
                            dows: vec![CrontabValue::Number(1)],
                        },
                        task_identifier: String::from("send_weekly_email"),
                        options: CrontabOptions::default(),
                        payload: None,
                    },
                    Crontab {
                        timer: CrontabTimer {
                            minutes: vec![CrontabValue::Number(30)],
                            hours: vec![CrontabValue::Number(4), CrontabValue::Range(10, 15)],
                            days: vec![CrontabValue::Any],
                            months: vec![CrontabValue::Any],
                            dows: vec![CrontabValue::Number(1)],
                        },
                        task_identifier: String::from("send_weekly_email"),
                        options: CrontabOptions {
                            fill: Some(CrontabFill {
                                s: 0,
                                m: 0,
                                h: 0,
                                d: 2,
                                w: 0,
                            }),
                            max: Some(10),
                            ..Default::default()
                        },
                        payload: Some(json!({ "onboarding": false })),
                    },
                    Crontab {
                        timer: CrontabTimer {
                            minutes: vec![CrontabValue::Number(0)],
                            hours: vec![CrontabValue::Step(4)],
                            days: vec![CrontabValue::Any],
                            months: vec![CrontabValue::Any],
                            dows: vec![CrontabValue::Any],
                        },
                        task_identifier: String::from("rollup"),
                        options: CrontabOptions::default(),
                        payload: None,
                    },
                ]
            )),
            nom_crontab(input)
        );
    }
}

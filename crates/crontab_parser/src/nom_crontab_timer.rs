use nom::{
    branch::alt,
    character::complete::{self, char},
    combinator::{map, opt, verify},
    multi::separated_list1,
    sequence::{preceded, separated_pair, terminated},
    IResult,
};

use crate::types::{CrontabPart, CrontabTimer, CrontabValue};

/// Attempts to parse a number with crontab part boundaries
fn crontab_number<'a>(part: &CrontabPart) -> impl Fn(&'a str) -> IResult<&'a str, u8> {
    let (min, max) = part.boundaries();
    move |input| verify(complete::u8, |v| v >= &min && v <= &max)(input)
}

/// Attempts to parse a range with crontab part boundaries
fn crontab_range<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, (u8, u8)> + 'p {
    |input| {
        verify(
            separated_pair(crontab_number(part), char('-'), crontab_number(part)),
            |(left, right)| left < right,
        )(input)
    }
}

/// Attempts to parse a step with crontab part boundaries
fn crontab_wildcard<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, Option<u8>> + 'p {
    |input| preceded(char('*'), opt(preceded(char('/'), crontab_number(part))))(input)
}

/// Attempts to parse a crontab part
fn crontab_value<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, CrontabValue> + 'p {
    |input| {
        alt((
            map(crontab_range(part), |(left, right)| {
                CrontabValue::Range(left, right)
            }),
            map(crontab_wildcard(part), |divider| match divider {
                Some(d) => CrontabValue::Step(d),
                None => CrontabValue::Any,
            }),
            map(crontab_number(part), CrontabValue::Number),
        ))(input)
    }
}

/// Attempts to parse comma separated crontab values
fn crontab_values<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, Vec<CrontabValue>> + 'p {
    |input| separated_list1(char(','), crontab_value(part))(input)
}

/// Parse all 5 crontab values
pub(crate) fn nom_crontab_timer(input: &str) -> IResult<&str, CrontabTimer> {
    let (input, minutes) = terminated(crontab_values(&CrontabPart::Minute), char(' '))(input)?;
    let (input, hours) = terminated(crontab_values(&CrontabPart::Hours), char(' '))(input)?;
    let (input, days) = terminated(crontab_values(&CrontabPart::Days), char(' '))(input)?;
    let (input, months) = terminated(crontab_values(&CrontabPart::Months), char(' '))(input)?;
    let (input, dows) = crontab_values(&CrontabPart::DaysOfWeek)(input)?;

    Ok((
        input,
        CrontabTimer {
            minutes,
            hours,
            days,
            months,
            dows,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crontab_timer_test_all_wildcard() {
        assert_eq!(
            Ok((
                " foo",
                CrontabTimer {
                    minutes: vec![CrontabValue::Any],
                    hours: vec![CrontabValue::Any],
                    days: vec![CrontabValue::Any],
                    months: vec![CrontabValue::Any],
                    dows: vec![CrontabValue::Any],
                }
            )),
            nom_crontab_timer("* * * * * foo"),
        );
    }

    #[test]
    fn crontab_timer_test_complex_comma_separated_list() {
        assert_eq!(
            Ok((
                " bar",
                CrontabTimer {
                    minutes: vec![
                        CrontabValue::Step(7),
                        CrontabValue::Number(8),
                        CrontabValue::Range(30, 35)
                    ],
                    hours: vec![CrontabValue::Any],
                    days: vec![CrontabValue::Number(3), CrontabValue::Step(4)],
                    months: vec![CrontabValue::Any],
                    dows: vec![CrontabValue::Any, CrontabValue::Number(4)],
                }
            )),
            nom_crontab_timer("*/7,8,30-35 * 3,*/4 * *,4 bar"),
        );
    }

    #[test]
    fn crontab_timer_test_error() {
        let timer_result = nom_crontab_timer("*/7!,8,30-35 * 3,*/4 * *,4 bar");
        assert!(timer_result.is_err());
    }
}

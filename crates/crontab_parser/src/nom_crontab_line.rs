use nom::{
    character::complete::digit1,
    combinator::{map_res, recognize, verify},
    error::context,
    error::ParseError,
    number::complete,
    IResult,
};
use thiserror::Error;

use crate::types::{CrontabPart, CrontabTimerValue};

/// Attempts to parse a number with crontab part boundaries
pub fn crontab_number<'a>(part: &CrontabPart) -> impl Fn(&'a str) -> IResult<&'a str, u8> {
    let (min, max) = part.boundaries();
    move |input| {
        verify(map_res(recognize(digit1), str::parse::<u8>), |v| {
            v >= &min && v <= &max
        })(input)
    }
}

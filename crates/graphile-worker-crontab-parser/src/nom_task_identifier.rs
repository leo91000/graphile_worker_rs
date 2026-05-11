use nom::{character::complete::satisfy, multi::many0, IResult, Parser};

pub(crate) fn nom_task_identifier(input: &str) -> IResult<&str, String> {
    let (input, first_char) = satisfy(|c| c.is_ascii_alphabetic()).parse(input)?;
    let (input, mut task_identifier) = many0(satisfy(|c| {
        c.is_ascii_alphanumeric() || c == ':' || c == '_' || c == '-'
    }))
    .parse(input)?;

    task_identifier.insert(0, first_char);
    Ok((input, task_identifier.iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_task_identifier() {
        assert_eq!(
            Ok((" foo", String::from("sendMail"))),
            nom_task_identifier("sendMail foo")
        );
        assert_eq!(
            Ok((" foo", String::from("mail:send_one"))),
            nom_task_identifier("mail:send_one foo")
        );
    }

    #[test]
    fn test_err_task_identifier_not_starting_with_alphatic_ascii_char() {
        let ti_result = nom_task_identifier("0:send_email foo");
        assert!(ti_result.is_err());
    }
}

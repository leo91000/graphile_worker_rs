use nom::{bytes::complete::take_while1, IResult};

pub(crate) fn nom_crontab_payload(input: &str) -> IResult<&str, serde_json::Value> {
    let (input, json) = take_while1(|c: char| c != '\n' && c != '\r')(input)?;

    let json: serde_json::Value = json5::from_str(json)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))?;

    Ok((input, json))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_valid_payload() {
        let input = "{onboarding:false,name:\"Jean\"} \nfoo";

        assert_eq!(
            Ok(("\nfoo", json!({ "onboarding": false, "name": "Jean" }),)),
            nom_crontab_payload(input)
        );

        let input = "{toggle:    4 ,name: 'Jean Pierre'}  ";

        assert_eq!(
            Ok(("", json!({ "toggle": 4, "name": "Jean Pierre" }),)),
            nom_crontab_payload(input)
        );
    }
}

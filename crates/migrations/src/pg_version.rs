use indoc::formatdoc;
use sqlx::{query, PgExecutor, Row};

use crate::MigrateError;

/// Fetches the postgres version and checks if it is compatible with Garphile Worker
pub async fn fetch_and_check_postgres_version<'e, E>(executor: E) -> Result<u32, MigrateError>
where
    E: PgExecutor<'e>,
{
    let sql = formatdoc!(
        r#"
            select current_setting('server_version_num') as server_version_num
        "#
    );

    let row = query(&sql).fetch_one(executor).await?;
    let version_string: String = row.try_get("server_version_num")?;

    check_postgres_version(&version_string)
}

/// Checks if the given postgres version is compatible with Graphile Worker
pub fn check_postgres_version(version_string: &str) -> Result<u32, MigrateError> {
    let version = version_string.parse::<u32>()?;

    if version < 120000 {
        return Err(MigrateError::IncompatibleVersion(version));
    }

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_postgres_version() {
        let version = check_postgres_version("120000").unwrap();
        assert_eq!(version, 120000);

        let version = check_postgres_version("120001").unwrap();
        assert_eq!(version, 120001);

        let version = check_postgres_version("120100").unwrap();
        assert_eq!(version, 120100);

        let version = check_postgres_version("120999").unwrap();
        assert_eq!(version, 120999);

        let version = check_postgres_version("130000").unwrap();
        assert_eq!(version, 130000);

        let version = check_postgres_version("130001").unwrap();
        assert_eq!(version, 130001);

        let version = check_postgres_version("130100").unwrap();
        assert_eq!(version, 130100);

        let version = check_postgres_version("130999").unwrap();
        assert_eq!(version, 130999);
    }

    #[test]
    fn test_check_postgres_version_error() {
        let version = check_postgres_version("119999");
        assert!(matches!(
            version,
            Err(MigrateError::IncompatibleVersion(119999))
        ));

        let version = check_postgres_version("110000");
        assert!(matches!(
            version,
            Err(MigrateError::IncompatibleVersion(110000))
        ));

        let version = check_postgres_version("foo");
        assert!(matches!(version, Err(MigrateError::ParseVersionError(_))));
    }
}

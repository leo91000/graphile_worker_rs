use chrono::prelude::*;
use getset::Getters;
use graphile_worker_database::{DbError, DbExecutorArg, DbParams, DbValue, Schema};
use indoc::formatdoc;

#[cfg_attr(feature = "driver-sqlx", derive(sqlx::FromRow))]
#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct KnownCrontab {
    identifier: String,
    known_since: DateTime<Local>,
    last_execution: Option<DateTime<Local>>,
}

impl KnownCrontab {
    pub fn new(
        identifier: String,
        known_since: DateTime<Local>,
        last_execution: Option<DateTime<Local>>,
    ) -> Self {
        Self {
            identifier,
            known_since,
            last_execution,
        }
    }
}

pub(crate) async fn get_known_crontabs(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
) -> Result<Vec<KnownCrontab>, DbError> {
    let known_crontabs = schema.private_table("known_crontabs");
    let sql = formatdoc!(
        r#"
            select * from {known_crontabs}
        "#
    );

    let rows = executor.fetch_all(&sql, DbParams::new()).await?;
    let known_crontabs = rows
        .into_iter()
        .map(|row| {
            Ok(KnownCrontab::new(
                row.try_get("identifier")?,
                row.try_get("known_since")?,
                row.try_get("last_execution")?,
            ))
        })
        .collect::<Result<Vec<_>, DbError>>()?;

    Ok(known_crontabs)
}

pub(crate) async fn insert_unknown_crontabs<Tz: TimeZone, S: AsRef<str>>(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    unknown_identifiers: &[S],
    start_time: &DateTime<Tz>,
) -> Result<(), DbError>
where
    Tz::Offset: Send + Sync,
{
    let known_crontabs = schema.private_table("known_crontabs");
    let sql = formatdoc!(
        r#"
            INSERT INTO {known_crontabs} (identifier, known_since)
            SELECT identifier, $2
            FROM unnest($1::text[]) AS unnest (identifier)
            ON CONFLICT DO NOTHING
        "#
    );

    let unknown_identifiers: Vec<String> = unknown_identifiers
        .iter()
        .map(|s| s.as_ref().to_string())
        .collect();

    executor
        .execute(
            &sql,
            vec![
                DbValue::TextArray(unknown_identifiers),
                DbValue::TimestampTz(start_time.with_timezone(&Utc)),
            ]
            .into(),
        )
        .await?;

    Ok(())
}

use crate::errors::ArchimedesError;
use chrono::Utc;
use getset::Getters;
use sqlx::{query, PgExecutor};

#[derive(Debug, Default)]
pub enum JobKeyMode {
    #[default]
    Replace,
    PreserveRunAt,
    UnsafeDedupe,
}

impl JobKeyMode {
    fn format(&self) -> &str {
        match self {
            JobKeyMode::Replace => "replace",
            JobKeyMode::PreserveRunAt => "preserve_run_at",
            JobKeyMode::UnsafeDedupe => "unsafe_dedupe",
        }
    }
}

#[derive(Getters, Debug, Default)]
pub struct JobSpec {
    pub queue_name: Option<String>,
    pub run_at: Option<chrono::DateTime<Utc>>,
    pub max_attempts: Option<i16>,
    pub job_key: Option<String>,
    pub job_key_mode: Option<JobKeyMode>,
    pub priority: Option<i16>,
    pub flags: Option<Vec<String>>,
}

pub async fn add_job(
    executor: impl for<'e> PgExecutor<'e>,
    escaped_schema: &str,
    identifier: &str,
    payload: serde_json::Value,
    spec: JobSpec,
) -> Result<(), ArchimedesError> {
    let sql = format!(
        r#"
            select * from {escaped_schema}.add_job(
                identifier => $1::text,
                payload => $2::json,
                queue_name => $3::text,
                run_at => $4::timestamptz,
                max_attempts => $5::smallint,
                job_key => $6::text,
                priority => $7::smallint,
                flags => $8::text[],
                job_key_mode => $9::text
            );
        "#
    );

    let job_key_mode = spec.job_key_mode.map(|jkm| jkm.format().to_string());

    query(&sql)
        .bind(identifier)
        .bind(payload)
        .bind(spec.queue_name)
        .bind(spec.run_at)
        .bind(spec.max_attempts)
        .bind(spec.job_key)
        .bind(spec.priority)
        .bind(spec.flags)
        .bind(job_key_mode)
        .execute(executor)
        .await?;

    Ok(())
}

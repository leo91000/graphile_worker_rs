use super::super::*;

pub(crate) fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set")
}

pub(crate) fn timestamp() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 9, 10, 11, 12)
        .single()
        .expect("valid timestamp")
}

pub(crate) fn unique_channel(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("{prefix}_{nanos}")
}

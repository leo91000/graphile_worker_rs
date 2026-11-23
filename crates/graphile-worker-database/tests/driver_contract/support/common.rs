use super::super::*;

pub(crate) fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set")
}

pub(crate) fn timestamp() -> OffsetDateTime {
    datetime!(2026-05-09 10:11:12.123456 -04:00)
}

pub(crate) fn unique_channel(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("{prefix}_{nanos}")
}

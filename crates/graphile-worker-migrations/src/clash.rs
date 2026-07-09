use graphile_worker_database::DbError;

const CLASH_CODES: &[&str] = &["23505", "42P06", "42P07", "42710"];

pub(crate) fn is_clash_error(error: &DbError) -> bool {
    error.code().is_some_and(|code| CLASH_CODES.contains(&code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_concurrent_migration_clash_codes() {
        for code in CLASH_CODES {
            assert!(is_clash_error(&DbError::with_code("clash", *code)));
        }
    }

    #[test]
    fn rejects_unrelated_database_errors() {
        assert!(!is_clash_error(&DbError::with_code("locked jobs", "22012")));
        assert!(!is_clash_error(&DbError::new("unknown database error")));
    }
}

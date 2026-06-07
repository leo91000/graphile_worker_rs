pub use graphile_worker_migrations_core::GraphileWorkerMigration;
use graphile_worker_migrations_macros::include_migrations;

pub const GRAPHILE_WORKER_MIGRATIONS: &[GraphileWorkerMigration] = include_migrations!("src/sql");

#[cfg(test)]
mod tests {
    use super::GRAPHILE_WORKER_MIGRATIONS;

    #[test]
    fn generated_migrations_are_ordered() {
        assert_eq!(GRAPHILE_WORKER_MIGRATIONS.len(), 20);
        for (index, migration) in GRAPHILE_WORKER_MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.migration_number(), index as u32 + 1);
            assert!(!migration.stmts.is_empty());
        }
    }

    #[test]
    fn generated_migrations_load_breaking_markers() {
        assert_eq!(
            GRAPHILE_WORKER_MIGRATIONS
                .iter()
                .filter(|migration| migration.is_breaking())
                .map(|migration| migration.migration_number())
                .collect::<Vec<_>>(),
            vec![1, 3, 11, 13, 14, 16]
        );
    }

    #[test]
    fn generated_migration_statements_do_not_include_metadata() {
        assert!(GRAPHILE_WORKER_MIGRATIONS.iter().all(|migration| migration
            .stmts
            .iter()
            .all(|stmt| !stmt.contains("-- graphile-worker-rs:"))));
    }
}

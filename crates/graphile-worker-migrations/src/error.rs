use graphile_worker_database::DbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrateError {
    #[error("Error occurred while parsing postgres version: {0}")]
    ParseVersionError(#[from] std::num::ParseIntError),
    #[error("This version of Graphile Worker requires PostgreSQL v12.0 or greater (detected `server_version_num` = {0})")]
    IncompatibleVersion(u32),
    #[error("Database is using Graphile Worker schema revision {} which includes breaking migration {}, but the currently running worker only supports up to revision {}. It would be unsafe to continue; please ensure all versions of Graphile Worker are compatible.", .latest_migration, .latest_breaking_migration, .highest_migration)]
    IncompatbleRevision {
        latest_migration: i32,
        latest_breaking_migration: i32,
        highest_migration: u32,
    },
    #[error("Error occurred while migrate: {0}")]
    SqlError(#[from] DbError),
    #[error("There are locked jobs present; migration 11 cannot complete. Please ensure all workers are shut down cleanly and all locked jobs and queues are unlocked before attempting this migration.")]
    LockedJobInMigration11,
}

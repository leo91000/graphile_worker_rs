use graphile_worker_database::{BoxFuture, DbError, DbExecutor, Schema};
use graphile_worker_extensions::Extensions;

/// A phase of one migration run.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum MigrationPhase<'a> {
    BeforeBootstrap,
    AfterBootstrap,
    BeforeMigrate,
    AfterMigrate,
    /// A revision failed and its transaction has been rolled back.
    RevisionFailed {
        revision: u32,
        error: &'a DbError,
    },
}

/// Shared session and typed, per-invocation state for migration callbacks.
pub struct MigrationEvent<'a> {
    pub phase: MigrationPhase<'a>,
    pub schema: &'a Schema,
    pub postgres_version: u32,
    pub executor: &'a dyn DbExecutor,
    pub scratchpad: &'a mut Extensions,
}

type Callback =
    dyn for<'a> Fn(MigrationEvent<'a>) -> BoxFuture<'a, Result<(), DbError>> + Send + Sync;

/// Optional callbacks run in registration order on the migration session.
///
/// Callbacks may set session options or use temporary tables. The migrator owns
/// transaction boundaries. Use a direct PostgreSQL connection or session-mode
/// pool so the same backend serves each phase. Callback writes outside a
/// revision transaction are committed normally and may outlive a failed run.
#[derive(Default)]
pub struct MigrationHooks {
    callbacks: Vec<Box<Callback>>,
}

impl MigrationHooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(
        mut self,
        callback: impl for<'a> Fn(MigrationEvent<'a>) -> BoxFuture<'a, Result<(), DbError>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.callbacks.push(Box::new(callback));
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }

    async fn emit(
        &self,
        phase: MigrationPhase<'_>,
        schema: &Schema,
        postgres_version: u32,
        executor: &dyn DbExecutor,
        scratchpad: &mut Extensions,
    ) -> Result<(), DbError> {
        for callback in &self.callbacks {
            callback(MigrationEvent {
                phase,
                schema,
                postgres_version,
                executor,
                scratchpad,
            })
            .await?;
        }
        Ok(())
    }
}

pub(super) struct MigrationRun<'a> {
    pub(super) connection: crate::connection::MigrationConnection<'a>,
    pub(super) hooks: &'a MigrationHooks,
    pub(super) postgres_version: u32,
    pub(super) scratchpad: Extensions,
}

impl MigrationRun<'_> {
    pub(super) async fn emit(
        &mut self,
        schema: &Schema,
        phase: MigrationPhase<'_>,
    ) -> Result<(), DbError> {
        self.hooks
            .emit(
                phase,
                schema,
                self.postgres_version,
                self.connection.executor(),
                &mut self.scratchpad,
            )
            .await
    }
}

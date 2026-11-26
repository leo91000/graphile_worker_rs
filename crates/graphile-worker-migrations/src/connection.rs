use graphile_worker_database::{Database, DbError, DbExecutor, DbParams, DbTransaction};

pub(super) enum MigrationConnection<'a> {
    Pooled(&'a Database),
    Pinned(Box<dyn DbExecutor>),
}

impl MigrationConnection<'_> {
    pub(super) fn executor(&self) -> &dyn DbExecutor {
        match self {
            Self::Pooled(database) => *database,
            Self::Pinned(executor) => executor.as_ref(),
        }
    }

    pub(super) async fn begin(&self) -> Result<MigrationTransaction<'_>, DbError> {
        match self {
            Self::Pooled(database) => Ok(MigrationTransaction::Pooled(database.begin().await?)),
            Self::Pinned(executor) => {
                executor.execute("BEGIN", DbParams::new()).await?;
                Ok(MigrationTransaction::Pinned(executor.as_ref()))
            }
        }
    }
}

pub(super) enum MigrationTransaction<'a> {
    Pooled(DbTransaction),
    Pinned(&'a dyn DbExecutor),
}

impl MigrationTransaction<'_> {
    pub(super) fn executor(&self) -> &dyn DbExecutor {
        match self {
            Self::Pooled(transaction) => transaction,
            Self::Pinned(executor) => *executor,
        }
    }

    /// The outer error indicates rollback failed, so callers must abandon the session.
    pub(super) async fn finish(
        self,
        result: Result<(), DbError>,
    ) -> Result<Result<(), DbError>, DbError> {
        match self {
            Self::Pooled(transaction) => Ok(match result {
                Ok(()) => transaction.commit().await,
                Err(error) => Err(error),
            }),
            Self::Pinned(executor) => {
                let result = match result {
                    Ok(()) => executor.execute("COMMIT", DbParams::new()).await.map(drop),
                    error => error,
                };
                if let Err(error) = &result {
                    if let Err(rollback) = executor.execute("ROLLBACK", DbParams::new()).await {
                        return Err(DbError::new(format!(
                            "{error}; rollback failed: {rollback}"
                        )));
                    }
                }
                Ok(result)
            }
        }
    }
}

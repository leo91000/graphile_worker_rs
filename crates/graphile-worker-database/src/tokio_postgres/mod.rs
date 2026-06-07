mod executor;
mod listener;
mod params;
mod rows;
mod transaction;

use ::tokio_postgres::NoTls;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};

use self::transaction::TokioPostgresTransaction;
use crate::{Database, DatabaseDriver, DbError, DbTransaction, NotificationStream};

#[derive(Clone, Debug)]
pub struct TokioPostgresDatabase {
    pool: Pool,
    config: Option<::tokio_postgres::Config>,
}

impl TokioPostgresDatabase {
    pub fn new(pool: Pool) -> Self {
        Self { pool, config: None }
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub fn from_config(config: ::tokio_postgres::Config, max_size: usize) -> Result<Self, DbError> {
        let manager = Manager::from_config(
            config.clone(),
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let pool = Pool::builder(manager)
            .max_size(max_size)
            .build()
            .map_err(|error| DbError::new(error.to_string()))?;
        Ok(Self {
            pool,
            config: Some(config),
        })
    }

    pub fn from_url(url: &str, max_size: usize) -> Result<Self, DbError> {
        let config = url
            .parse::<::tokio_postgres::Config>()
            .map_err(|error| DbError::new(error.to_string()))?;
        Self::from_config(config, max_size)
    }
}

impl From<TokioPostgresDatabase> for Database {
    fn from(database: TokioPostgresDatabase) -> Self {
        Database::new(database)
    }
}

impl From<Pool> for TokioPostgresDatabase {
    fn from(pool: Pool) -> Self {
        Self::new(pool)
    }
}

impl From<Pool> for Database {
    fn from(pool: Pool) -> Self {
        Database::new(TokioPostgresDatabase::new(pool))
    }
}

impl DatabaseDriver for TokioPostgresDatabase {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn begin<'a>(&'a self) -> crate::BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async move {
            let client = self.pool.get().await?;
            client.batch_execute("BEGIN").await?;
            Ok(DbTransaction::new(Box::new(TokioPostgresTransaction::new(
                client,
            ))))
        })
    }

    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> crate::BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async move { listener::listen(self.config.clone(), channel).await })
    }
}

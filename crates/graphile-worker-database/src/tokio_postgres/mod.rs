mod executor;
mod listener;
mod params;
mod rows;
mod transaction;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use graphile_worker_postgres_tls as tls;

use self::transaction::TokioPostgresTransaction;
use crate::{Database, DatabaseDriver, DbError, DbTransaction, NotificationStream};

#[derive(Clone)]
pub struct TokioPostgresDatabase {
    pool: Pool,
    listener_config: Option<ListenerConfig>,
}

#[derive(Clone)]
struct ListenerConfig {
    config: ::tokio_postgres::Config,
    tls: tls::Connector,
}

impl std::fmt::Debug for TokioPostgresDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokioPostgresDatabase")
            .field("pool", &self.pool)
            .field("has_listener", &self.listener_config.is_some())
            .finish()
    }
}

impl TokioPostgresDatabase {
    /// Wraps a caller-built pool without a dedicated LISTEN connection.
    /// Use [`Self::from_config`] or [`Self::from_url`] to enable notifications.
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            listener_config: None,
        }
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Creates a pool and listener using the selected TLS feature.
    /// Native TLS takes precedence when both TLS features are enabled.
    /// `PGSSLROOTCERT` may name a PEM file containing additional trusted roots.
    #[allow(clippy::clone_on_copy)] // NoTls is Copy; TLS connectors are only Clone.
    pub fn from_config(config: ::tokio_postgres::Config, max_size: usize) -> Result<Self, DbError> {
        let root_cert = std::env::var_os("PGSSLROOTCERT");
        let tls = tls::connector(root_cert.as_deref().map(std::path::Path::new))
            .map_err(|error| DbError::new(error.to_string()))?;
        // The no-TLS connector is Copy; both TLS connectors require cloning.
        #[allow(clippy::clone_on_copy)]
        let pool_tls = tls.clone();
        let manager = Manager::from_config(
            config.clone(),
            pool_tls,
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
            listener_config: Some(ListenerConfig { config, tls }),
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
        Box::pin(async move { listener::listen(self.listener_config.clone(), channel).await })
    }
}

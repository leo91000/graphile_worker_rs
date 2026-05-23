use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use serde_json::Value;
use tokio::sync::{mpsc::UnboundedSender, oneshot, Mutex};
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{AsyncMessage, Client, NoTls, Row, Transaction};

use crate::{
    Database, DatabaseDriver, DbCell, DbError, DbExecutor, DbExecutorArg, DbParams, DbRow,
    DbTransaction, DbValue, Notification, NotificationStream, TransactionDriver,
};

const INITIAL_LISTENER_RECONNECT_DELAY: Duration = Duration::from_millis(50);
const MAX_LISTENER_RECONNECT_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct TokioPostgresDatabase {
    pool: Pool,
    config: Option<tokio_postgres::Config>,
}

impl TokioPostgresDatabase {
    pub fn new(pool: Pool) -> Self {
        Self { pool, config: None }
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub fn from_config(config: tokio_postgres::Config, max_size: usize) -> Result<Self, DbError> {
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
            .parse::<tokio_postgres::Config>()
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

impl From<tokio_postgres::Error> for DbError {
    fn from(error: tokio_postgres::Error) -> Self {
        if let Some(db_error) = error.as_db_error() {
            return DbError::with_code(error.to_string(), db_error.code().code());
        }

        DbError::new(error.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for DbError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        DbError::new(error.to_string())
    }
}

fn boxed_param(value: DbValue) -> Box<dyn ToSql + Sync + Send> {
    match value {
        DbValue::Bool(value) => Box::new(value),
        DbValue::BoolOpt(value) => Box::new(value),
        DbValue::I16(value) => Box::new(value),
        DbValue::I16Opt(value) => Box::new(value),
        DbValue::I32(value) => Box::new(value),
        DbValue::I32Opt(value) => Box::new(value),
        DbValue::I64(value) => Box::new(value),
        DbValue::I64Opt(value) => Box::new(value),
        DbValue::Json(value) => Box::new(value),
        DbValue::JsonOpt(value) => Box::new(value),
        DbValue::Text(value) => Box::new(value),
        DbValue::TextOpt(value) => Box::new(value),
        DbValue::TextArray(value) => Box::new(value),
        DbValue::TextArrayOpt(value) => Box::new(value),
        DbValue::I32Array(value) => Box::new(value),
        DbValue::I64Array(value) => Box::new(value),
        DbValue::TimestampTz(value) => Box::new(value),
        DbValue::TimestampTzOpt(value) => Box::new(value),
    }
}

fn boxed_params(params: DbParams) -> Vec<Box<dyn ToSql + Sync + Send>> {
    params.values().iter().cloned().map(boxed_param).collect()
}

fn param_refs(params: &[Box<dyn ToSql + Sync + Send>]) -> Vec<&(dyn ToSql + Sync)> {
    params
        .iter()
        .map(|param| param.as_ref() as &(dyn ToSql + Sync))
        .collect()
}

fn tokio_row_to_db_row(row: Row) -> Result<DbRow, DbError> {
    let mut cells = HashMap::with_capacity(row.columns().len());

    for (index, column) in row.columns().iter().enumerate() {
        let name = column.name().to_string();
        let cell = match *column.type_() {
            Type::BOOL => row
                .try_get::<usize, Option<bool>>(index)?
                .map(DbCell::Bool)
                .unwrap_or(DbCell::Null),
            Type::INT2 => row
                .try_get::<usize, Option<i16>>(index)?
                .map(DbCell::I16)
                .unwrap_or(DbCell::Null),
            Type::INT4 => row
                .try_get::<usize, Option<i32>>(index)?
                .map(DbCell::I32)
                .unwrap_or(DbCell::Null),
            Type::INT8 => row
                .try_get::<usize, Option<i64>>(index)?
                .map(DbCell::I64)
                .unwrap_or(DbCell::Null),
            Type::JSON | Type::JSONB => row
                .try_get::<usize, Option<Value>>(index)?
                .map(DbCell::Json)
                .unwrap_or(DbCell::Null),
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => row
                .try_get::<usize, Option<String>>(index)?
                .map(DbCell::Text)
                .unwrap_or(DbCell::Null),
            Type::TIMESTAMPTZ => row
                .try_get::<usize, Option<DateTime<Utc>>>(index)?
                .map(DbCell::TimestampTz)
                .unwrap_or(DbCell::Null),
            ref other => {
                return Err(DbError::new(format!(
                    "unsupported PostgreSQL result type `{other}` for column `{name}`"
                )));
            }
        };
        cells.insert(name, cell);
    }

    Ok(DbRow::new(cells))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn next_listener_reconnect_delay(delay: Duration) -> Duration {
    let doubled = delay.checked_mul(2).unwrap_or(MAX_LISTENER_RECONNECT_DELAY);

    if doubled > MAX_LISTENER_RECONNECT_DELAY {
        MAX_LISTENER_RECONNECT_DELAY
    } else {
        doubled
    }
}

async fn connect_tokio_postgres_listener(
    config: &tokio_postgres::Config,
    listen_sql: &str,
    tx: &UnboundedSender<Result<Notification, DbError>>,
) -> Result<(Client, oneshot::Receiver<()>), DbError> {
    let (client, connection) = config.connect(NoTls).await?;
    let (closed_tx, closed_rx) = oneshot::channel();
    let tx = tx.clone();

    drop(tokio::spawn(async move {
        let mut connection = Box::pin(connection);

        while let Some(message) =
            std::future::poll_fn(|cx| connection.as_mut().poll_message(cx)).await
        {
            let item = match message {
                Ok(AsyncMessage::Notification(notification)) => Ok(Notification {
                    channel: notification.channel().to_string(),
                    payload: notification.payload().to_string(),
                }),
                Ok(AsyncMessage::Notice(_)) => continue,
                Ok(_) => continue,
                Err(_) => break,
            };

            if tx.send(item).is_err() {
                break;
            }
        }

        let _ = closed_tx.send(());
    }));

    client.batch_execute(listen_sql).await?;

    Ok((client, closed_rx))
}

async fn run_reconnecting_tokio_postgres_listener(
    config: tokio_postgres::Config,
    listen_sql: String,
    tx: UnboundedSender<Result<Notification, DbError>>,
    mut client: Client,
    mut closed_rx: oneshot::Receiver<()>,
) {
    let mut reconnect_delay = INITIAL_LISTENER_RECONNECT_DELAY;

    loop {
        tokio::select! {
            _ = tx.closed() => return,
            _ = &mut closed_rx => {}
        }

        drop(client);

        loop {
            tokio::select! {
                _ = tx.closed() => return,
                _ = tokio::time::sleep(reconnect_delay) => {}
            }

            match connect_tokio_postgres_listener(&config, &listen_sql, &tx).await {
                Ok((new_client, new_closed_rx)) => {
                    client = new_client;
                    closed_rx = new_closed_rx;
                    reconnect_delay = INITIAL_LISTENER_RECONNECT_DELAY;
                    break;
                }
                Err(_) => {
                    reconnect_delay = next_listener_reconnect_delay(reconnect_delay);
                }
            }
        }
    }
}

impl DbExecutor for TokioPostgresDatabase {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let client = self.pool.get().await?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            client.execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let client = self.pool.get().await?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = client.query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
    }
}

impl DbExecutorArg for &Client {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let params = boxed_params(params);
            let refs = param_refs(&params);
            (**self).execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = (**self).query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
    }
}

impl DbExecutorArg for &Transaction<'_> {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let params = boxed_params(params);
            let refs = param_refs(&params);
            (**self).execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = (**self).query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
    }
}

impl DbExecutorArg for &deadpool_postgres::Client {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let params = boxed_params(params);
            let refs = param_refs(&params);
            (**self).execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = (**self).query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
    }
}

impl DbExecutorArg for &Pool {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let client = self.get().await?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            client.execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a mut self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let client = self.get().await?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = client.query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
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
            Ok(DbTransaction::new(Box::new(TokioPostgresTransaction {
                client: Mutex::new(Some(client)),
            })))
        })
    }

    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> crate::BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async move {
            let Some(config) = self.config.clone() else {
                return Ok(None);
            };

            let sql = format!("LISTEN {}", quote_identifier(channel));
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let (client, closed_rx) = connect_tokio_postgres_listener(&config, &sql, &tx).await?;

            drop(tokio::spawn(run_reconnecting_tokio_postgres_listener(
                config, sql, tx, client, closed_rx,
            )));

            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

            Ok(Some(Box::pin(stream) as NotificationStream))
        })
    }
}

pub struct TokioPostgresTransaction {
    client: Mutex<Option<deadpool_postgres::Client>>,
}

impl DbExecutor for TokioPostgresTransaction {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            let mut guard = self.client.lock().await;
            let client = guard
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            client.execute(sql, &refs).await.map_err(Into::into)
        })
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move {
            let mut guard = self.client.lock().await;
            let client = guard
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            let params = boxed_params(params);
            let refs = param_refs(&params);
            let rows = client.query(sql, &refs).await?;
            rows.into_iter().map(tokio_row_to_db_row).collect()
        })
    }
}

impl TransactionDriver for TokioPostgresTransaction {
    fn commit(self: Box<Self>) -> crate::BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async move {
            let mut guard = self.client.lock().await;
            let client = guard
                .as_mut()
                .ok_or_else(|| DbError::new("transaction has already been committed"))?;
            client.batch_execute("COMMIT").await?;
            guard.take();
            Ok(())
        })
    }
}

impl Drop for TokioPostgresTransaction {
    fn drop(&mut self) {
        let Some(client) = self.client.get_mut().take() else {
            return;
        };
        drop(tokio::spawn(async move {
            let _ = client.batch_execute("ROLLBACK").await;
        }));
    }
}

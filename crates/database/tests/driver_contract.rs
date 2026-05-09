#![cfg(feature = "runtime-tokio")]

use std::any::Any;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, TimeZone, Utc};
use futures::StreamExt;
use graphile_worker_database::{
    row_mapping, Database, DatabaseDriver, DbCell, DbError, DbExecutor, DbParams, DbRow,
    DbTransaction, DbValue, NotificationStream, TransactionDriver,
};
use serde_json::json;

#[cfg(feature = "driver-sqlx")]
use graphile_worker_database::sqlx::SqlxDatabase;
#[cfg(feature = "driver-tokio-postgres")]
use graphile_worker_database::tokio_postgres::TokioPostgresDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set")
}

fn timestamp() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 9, 10, 11, 12)
        .single()
        .expect("valid timestamp")
}

fn unique_channel(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("{prefix}_{nanos}")
}

#[test]
fn db_row_decodes_supported_cells_and_errors() {
    let ts = timestamp();
    let row = row_mapping::cells([
        ("bool", DbCell::Bool(true)),
        ("i16", DbCell::I16(16)),
        ("i32", DbCell::I32(32)),
        ("i64", DbCell::I64(64)),
        ("json", DbCell::Json(json!({ "ok": true }))),
        ("text", DbCell::Text("value".to_string())),
        ("timestamp", DbCell::TimestampTz(ts)),
        ("null", DbCell::Null),
    ]);

    assert!(row.try_get::<bool>("bool").unwrap());
    assert_eq!(row.try_get::<i16>("i16").unwrap(), 16);
    assert_eq!(row.try_get::<i32>("i32").unwrap(), 32);
    assert_eq!(row.try_get::<i64>("i64").unwrap(), 64);
    assert_eq!(
        row.try_get::<serde_json::Value>("json").unwrap(),
        json!({ "ok": true })
    );
    assert_eq!(row.try_get::<String>("text").unwrap(), "value");
    assert_eq!(row.try_get::<DateTime<Utc>>("timestamp").unwrap(), ts);
    assert_eq!(
        row.try_get::<DateTime<Local>>("timestamp")
            .unwrap()
            .with_timezone(&Utc),
        ts
    );
    assert_eq!(row.try_get::<Option<bool>>("null").unwrap(), None);
    assert_eq!(
        row.try_get::<Option<String>>("text").unwrap(),
        Some("value".to_string())
    );

    let missing = row.try_get::<bool>("missing").unwrap_err();
    assert!(missing.to_string().contains("was not present"));

    let wrong_type = row.try_get::<i32>("text").unwrap_err();
    assert!(wrong_type
        .to_string()
        .contains("could not be decoded as i32"));

    let coded = DbError::with_code("duplicate key", "23505");
    assert_eq!(coded.code(), Some("23505"));
}

#[test]
fn db_params_preserve_values() {
    let mut params = DbParams::new();
    params.push(DbValue::Bool(true));
    params.push(DbValue::Text("hello".to_string()));

    assert_eq!(params.values().len(), 2);

    let from_vec = DbParams::from(vec![DbValue::I32(42)]);
    assert_eq!(from_vec.values().len(), 1);
}

#[derive(Debug)]
struct MockDriver {
    rows: Vec<DbRow>,
}

impl DbExecutor for MockDriver {
    #[cfg(feature = "driver-sqlx")]
    fn try_sqlx_pool(&self) -> Option<&::sqlx::PgPool> {
        None
    }

    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async { Ok(7) })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { Ok(self.rows.clone()) })
    }
}

impl DatabaseDriver for MockDriver {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn begin<'a>(
        &'a self,
    ) -> graphile_worker_database::BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async { Ok(DbTransaction::new(Box::new(MockTransaction))) })
    }

    fn listen<'a>(
        &'a self,
        _channel: &'a str,
    ) -> graphile_worker_database::BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async { Ok(None) })
    }
}

struct MockTransaction;

impl DbExecutor for MockTransaction {
    fn execute<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async { Ok(3) })
    }

    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { Ok(vec![row_mapping::cells([("value", DbCell::I32(99))])]) })
    }
}

impl TransactionDriver for MockTransaction {
    fn commit(
        self: Box<Self>,
    ) -> graphile_worker_database::BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn database_wrapper_delegates_to_inner_driver() {
    let database = Database::new(MockDriver {
        rows: vec![row_mapping::cells([("value", DbCell::I32(1))])],
    });
    let cloned = Database::from(&database);

    assert!(format!("{database:?}").contains("Database"));
    assert!(cloned.downcast_ref::<MockDriver>().is_some());
    #[cfg(feature = "driver-sqlx")]
    assert!(database.try_sqlx_pool().is_none());

    assert_eq!(
        database
            .fetch_one("select value", DbParams::new())
            .await
            .unwrap()
            .try_get::<i32>("value")
            .unwrap(),
        1
    );
    assert_eq!(
        database
            .fetch_optional("select value", DbParams::new())
            .await
            .unwrap()
            .expect("one row")
            .try_get::<i32>("value")
            .unwrap(),
        1
    );
    assert!(database.listen("events").await.unwrap().is_none());
    assert_eq!(
        database.execute("select 1", DbParams::new()).await.unwrap(),
        7
    );

    let tx = database.begin().await.unwrap();
    assert_eq!(
        tx.fetch_one("select value", DbParams::new())
            .await
            .unwrap()
            .try_get::<i32>("value")
            .unwrap(),
        99
    );
    assert_eq!(tx.execute("select 1", DbParams::new()).await.unwrap(), 3);
    tx.commit().await.unwrap();

    let empty_database = Database::new(MockDriver { rows: vec![] });
    assert!(empty_database
        .fetch_optional("select value", DbParams::new())
        .await
        .unwrap()
        .is_none());
    assert!(empty_database
        .fetch_one("select value", DbParams::new())
        .await
        .unwrap_err()
        .to_string()
        .contains("exactly one row"));
}

async fn exercise_executor(executor: &impl DbExecutor) {
    let ts = timestamp();
    let rows = executor
        .fetch_all(
            r#"
                select
                    $1::bool as bool_value,
                    $2::bool as bool_null,
                    $3::int2 as i16_value,
                    $4::int2 as i16_null,
                    $5::int4 as i32_value,
                    $6::int4 as i32_null,
                    $7::int8 as i64_value,
                    $8::int8 as i64_null,
                    $9::jsonb as json_value,
                    $10::jsonb as json_null,
                    $11::text as text_value,
                    $12::text as text_null,
                    array_length($13::text[], 1)::int4 as text_array_len,
                    array_length($14::text[], 1)::int4 as text_array_null_len,
                    array_length($15::int4[], 1)::int4 as i32_array_len,
                    array_length($16::int8[], 1)::int4 as i64_array_len,
                    $17::timestamptz as timestamp_value,
                    $18::timestamptz as timestamp_null
            "#,
            DbParams::from(vec![
                DbValue::Bool(true),
                DbValue::BoolOpt(None),
                DbValue::I16(16),
                DbValue::I16Opt(None),
                DbValue::I32(32),
                DbValue::I32Opt(None),
                DbValue::I64(64),
                DbValue::I64Opt(None),
                DbValue::Json(json!({ "driver": "covered" })),
                DbValue::JsonOpt(None),
                DbValue::Text("hello".to_string()),
                DbValue::TextOpt(None),
                DbValue::TextArray(vec!["a".to_string(), "b".to_string()]),
                DbValue::TextArrayOpt(None),
                DbValue::I32Array(vec![1, 2, 3]),
                DbValue::I64Array(vec![4, 5]),
                DbValue::TimestampTz(ts),
                DbValue::TimestampTzOpt(None),
            ]),
        )
        .await
        .unwrap();

    let row = rows.first().expect("one row");
    assert!(row.try_get::<bool>("bool_value").unwrap());
    assert_eq!(row.try_get::<Option<bool>>("bool_null").unwrap(), None);
    assert_eq!(row.try_get::<i16>("i16_value").unwrap(), 16);
    assert_eq!(row.try_get::<Option<i16>>("i16_null").unwrap(), None);
    assert_eq!(row.try_get::<i32>("i32_value").unwrap(), 32);
    assert_eq!(row.try_get::<Option<i32>>("i32_null").unwrap(), None);
    assert_eq!(row.try_get::<i64>("i64_value").unwrap(), 64);
    assert_eq!(row.try_get::<Option<i64>>("i64_null").unwrap(), None);
    assert_eq!(
        row.try_get::<serde_json::Value>("json_value").unwrap(),
        json!({ "driver": "covered" })
    );
    assert_eq!(
        row.try_get::<Option<serde_json::Value>>("json_null")
            .unwrap(),
        None
    );
    assert_eq!(row.try_get::<String>("text_value").unwrap(), "hello");
    assert_eq!(row.try_get::<Option<String>>("text_null").unwrap(), None);
    assert_eq!(row.try_get::<i32>("text_array_len").unwrap(), 2);
    assert_eq!(
        row.try_get::<Option<i32>>("text_array_null_len").unwrap(),
        None
    );
    assert_eq!(row.try_get::<i32>("i32_array_len").unwrap(), 3);
    assert_eq!(row.try_get::<i32>("i64_array_len").unwrap(), 2);
    assert_eq!(row.try_get::<DateTime<Utc>>("timestamp_value").unwrap(), ts);
    assert_eq!(
        row.try_get::<Option<DateTime<Utc>>>("timestamp_null")
            .unwrap(),
        None
    );

    let fetched = executor
        .fetch_optional("select 123::int4 as value", DbParams::new())
        .await
        .unwrap()
        .expect("one row");
    assert_eq!(fetched.try_get::<i32>("value").unwrap(), 123);

    let first = executor
        .fetch_optional(
            "select value::int4 from unnest(array[1, 2]) as values(value)",
            DbParams::new(),
        )
        .await
        .unwrap()
        .expect("one row");
    assert_eq!(first.try_get::<i32>("value").unwrap(), 1);

    let none = executor
        .fetch_optional("select 123::int4 as value where false", DbParams::new())
        .await
        .unwrap();
    assert!(none.is_none());

    assert!(executor
        .fetch_one("select 123::int4 as value where false", DbParams::new())
        .await
        .unwrap_err()
        .to_string()
        .contains("exactly one row"));

    assert!(executor
        .fetch_all("select 1.5::numeric as unsupported", DbParams::new())
        .await
        .unwrap_err()
        .to_string()
        .contains("unsupported PostgreSQL result type"));

    executor
        .execute("select $1::int4", DbParams::from(vec![DbValue::I32(1)]))
        .await
        .unwrap();
}

async fn exercise_database(database: &Database) {
    exercise_executor(database).await;

    let tx = database.begin().await.unwrap();
    tx.execute(
        "create temp table database_driver_contract(value int4) on commit drop",
        DbParams::new(),
    )
    .await
    .unwrap();
    tx.execute(
        "insert into database_driver_contract(value) values ($1)",
        DbParams::from(vec![DbValue::I32(42)]),
    )
    .await
    .unwrap();
    assert_eq!(
        tx.fetch_one(
            "select value from database_driver_contract",
            DbParams::new()
        )
        .await
        .unwrap()
        .try_get::<i32>("value")
        .unwrap(),
        42
    );
    tx.commit().await.unwrap();

    let tx = database.begin().await.unwrap();
    tx.execute("select 1", DbParams::new()).await.unwrap();
    drop(tx);
    tokio::time::sleep(Duration::from_millis(20)).await;
}

async fn exercise_listen(database: &Database, channel: &str) {
    let mut stream = database
        .listen(channel)
        .await
        .unwrap()
        .expect("driver should support notifications");

    database
        .execute(
            &format!("notify {channel}, 'driver-payload'"),
            DbParams::new(),
        )
        .await
        .unwrap();

    let notification = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("notification should arrive")
        .expect("notification stream should stay open")
        .unwrap();

    assert_eq!(notification.channel, channel);
    assert_eq!(notification.payload, "driver-payload");
}

#[cfg(feature = "driver-sqlx")]
#[tokio::test]
async fn sqlx_driver_satisfies_database_contract() {
    let pool = ::sqlx::PgPool::connect(&database_url()).await.unwrap();
    let sqlx_database = SqlxDatabase::new(pool.clone());

    assert!(fmt::format(format_args!("{sqlx_database:?}")).contains("SqlxDatabase"));
    assert!(!sqlx_database.pool().is_closed());

    let database = Database::from(sqlx_database.clone());
    assert!(database.downcast_ref::<SqlxDatabase>().is_some());
    assert!(database.try_sqlx_pool().is_some());

    exercise_database(&database).await;
    exercise_listen(&database, &unique_channel("database_driver_sqlx")).await;

    let from_pool = Database::from(pool.clone());
    assert!(from_pool.try_sqlx_pool().is_some());
    let from_pool_ref = Database::from(&pool);
    assert!(from_pool_ref.try_sqlx_pool().is_some());
}

#[cfg(feature = "driver-tokio-postgres")]
#[tokio::test]
async fn tokio_postgres_driver_satisfies_database_contract() {
    let tokio_database = TokioPostgresDatabase::from_url(&database_url(), 4).unwrap();

    assert!(fmt::format(format_args!("{tokio_database:?}")).contains("TokioPostgresDatabase"));

    let database = Database::from(tokio_database.clone());
    assert!(database.downcast_ref::<TokioPostgresDatabase>().is_some());
    #[cfg(feature = "driver-sqlx")]
    assert!(database.try_sqlx_pool().is_none());

    exercise_database(&database).await;
    exercise_listen(&database, &unique_channel("database_driver_tokio")).await;

    let pool_database = TokioPostgresDatabase::new(tokio_database.pool().clone());
    assert!(pool_database
        .listen("without_config")
        .await
        .unwrap()
        .is_none());

    let from_pool = Database::from(tokio_database.pool().clone());
    exercise_executor(&from_pool).await;
}

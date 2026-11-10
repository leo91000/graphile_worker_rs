use super::super::*;
use super::common::timestamp;

pub(crate) async fn exercise_executor(executor: &impl DbExecutor) {
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

/// Checks values, server errors, transactions and cleanup against a real driver.
pub(crate) async fn exercise_database(database: &Database) {
    exercise_executor(database).await;

    // Both drivers must preserve the server message as well as SQLSTATE, even
    // when the driver's outer error display contains only "db error".
    let reject = "do $$ begin raise exception using errcode='40001', message='driver contract failure', detail='driver contract detail'; end $$";
    let error = database.execute(reject, DbParams::new()).await.unwrap_err();
    assert_eq!(error.code(), Some("40001"));
    assert!(error.to_string().contains("driver contract failure"));
    #[cfg(feature = "driver-tokio-postgres")]
    if database.downcast_ref::<TokioPostgresDatabase>().is_some() {
        assert_eq!(
            native_server_diagnostic(&error),
            Some((
                "40001",
                "driver contract failure",
                Some("driver contract detail"),
                Some(::tokio_postgres::error::Severity::Error)
            ))
        );
    }

    let tx = database.begin().await.unwrap();
    let error = tx.fetch_all(reject, DbParams::new()).await.unwrap_err();
    assert_eq!(error.code(), Some("40001"));
    assert!(error.to_string().contains("driver contract failure"));
    #[cfg(feature = "driver-tokio-postgres")]
    if database.downcast_ref::<TokioPostgresDatabase>().is_some() {
        assert_eq!(
            native_server_diagnostic(&error),
            Some((
                "40001",
                "driver contract failure",
                Some("driver contract detail"),
                Some(::tokio_postgres::error::Severity::Error)
            ))
        );
    }
    drop(tx);

    #[cfg(feature = "driver-tokio-postgres")]
    if database.downcast_ref::<TokioPostgresDatabase>().is_some() {
        let proof = verify_missing_database_pool_refusal().await;
        assert!(
            proof.is_ok(),
            "pool refusal did not retain its native cause: {proof:?}"
        );
    }

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

#[cfg(feature = "driver-tokio-postgres")]
fn native_server_diagnostic(
    error: &DbError,
) -> Option<(
    &str,
    &str,
    Option<&str>,
    Option<::tokio_postgres::error::Severity>,
)> {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current_source) = source {
        match current_source.downcast_ref::<::tokio_postgres::Error>() {
            Some(driver_error) => {
                return driver_error.as_db_error().map(|server_error| {
                    (
                        server_error.code().code(),
                        server_error.message(),
                        server_error.detail(),
                        server_error.parsed_severity(),
                    )
                })
            }
            None => source = current_source.source(),
        }
    }
    None
}

#[cfg(feature = "driver-tokio-postgres")]
async fn verify_missing_database_pool_refusal(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut config = match database_url().parse::<::tokio_postgres::Config>() {
        Ok(config) => config,
        Err(error) => return Err(error.into()),
    };
    let missing_database = unique_channel("driver_contract_missing");
    config.dbname(&missing_database);
    let database = match TokioPostgresDatabase::from_config(config, 1) {
        Ok(database) => Database::from(database),
        Err(error) => return Err(error.into()),
    };
    let error = match database.execute("select 1", DbParams::new()).await {
        Ok(_) => return Err("the fixture's nonexistent database was accepted".into()),
        Err(error) => error,
    };
    assert_eq!(error.code(), Some("3D000"));
    let assert_locale_stable_diagnostic = |error: &DbError| {
        let (code, message, detail, severity) =
            native_server_diagnostic(error).expect("native cause retained");
        assert_eq!(code, "3D000");
        assert!(
            message.contains(missing_database.as_str()),
            "server message should name the missing database: {message}"
        );
        assert_eq!(detail, None);
        assert_eq!(severity, Some(::tokio_postgres::error::Severity::Fatal));
    };
    assert_locale_stable_diagnostic(&error);
    assert_locale_stable_diagnostic(&error.clone());
    Ok(())
}

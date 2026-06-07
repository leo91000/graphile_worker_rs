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

pub(crate) async fn exercise_database(database: &Database) {
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

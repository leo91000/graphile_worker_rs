use super::*;

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

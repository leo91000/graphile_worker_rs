use super::*;

#[tokio::test]
async fn database_wrapper_delegates_to_inner_driver() {
    let database = Database::new(MockDriver {
        rows: vec![row_mapping::cells([("value", DbCell::I32(1))])],
    });
    let cloned = Database::from(&database);

    assert!(format!("{database:?}").contains("Database"));
    assert!(cloned.downcast_ref::<MockDriver>().is_some());

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

use super::*;

#[cfg(feature = "driver-sqlx")]
#[tokio::test]
async fn sqlx_driver_satisfies_database_contract() {
    let pool = ::sqlx::PgPool::connect(&database_url()).await.unwrap();
    let sqlx_database = SqlxDatabase::new(pool.clone());

    assert!(fmt::format(format_args!("{sqlx_database:?}")).contains("SqlxDatabase"));
    assert!(!sqlx_database.pool().is_closed());

    let database = Database::from(sqlx_database.clone());
    assert!(database.downcast_ref::<SqlxDatabase>().is_some());

    exercise_database(&database).await;
    exercise_listen(&database, &unique_channel("database_driver_sqlx")).await;

    let from_pool = Database::from(pool.clone());
    exercise_database(&from_pool).await;
    let from_pool_ref = Database::from(&pool);
    exercise_database(&from_pool_ref).await;
}
#[cfg(feature = "driver-tokio-postgres")]
#[tokio::test]
async fn tokio_postgres_driver_satisfies_database_contract() {
    let tokio_database = TokioPostgresDatabase::from_url(&database_url(), 4).unwrap();

    assert!(fmt::format(format_args!("{tokio_database:?}")).contains("TokioPostgresDatabase"));

    let database = Database::from(tokio_database.clone());
    assert!(database.downcast_ref::<TokioPostgresDatabase>().is_some());

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
#[cfg(feature = "driver-tokio-postgres")]
#[tokio::test]
async fn tokio_postgres_listener_reconnects_after_connection_loss() {
    let channel = unique_channel("database_driver_tokio_reconnect");
    let application_name = unique_channel("database_driver_tokio_listener");
    let listen_query = format!(
        "LISTEN {}",
        graphile_worker_database::escape_identifier(&channel)
    );
    let mut config = database_url().parse::<tokio_postgres::Config>().unwrap();
    config.application_name(&application_name);
    let tokio_database = TokioPostgresDatabase::from_config(config, 4).unwrap();
    let database = Database::from(tokio_database);

    let mut stream = database
        .listen(&channel)
        .await
        .unwrap()
        .expect("driver should support notifications");

    let first_pid =
        wait_for_tokio_postgres_listener_pid(&database, &application_name, &listen_query, None)
            .await;

    notify(&database, &channel, "before-reconnect").await;
    expect_notification(&mut stream, &channel, "before-reconnect").await;

    let terminated = database
        .fetch_one(
            "select pg_terminate_backend($1) as terminated",
            DbParams::from(vec![DbValue::I32(first_pid)]),
        )
        .await
        .unwrap();
    assert!(terminated.try_get::<bool>("terminated").unwrap());

    let _second_pid = wait_for_tokio_postgres_listener_pid(
        &database,
        &application_name,
        &listen_query,
        Some(first_pid),
    )
    .await;

    notify(&database, &channel, "after-reconnect").await;
    expect_notification(&mut stream, &channel, "after-reconnect").await;
}

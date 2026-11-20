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
async fn tokio_postgres_cancelling_begin_discards_the_unconfirmed_connection() {
    let database = Database::from(TokioPostgresDatabase::from_url(&database_url(), 1).unwrap());
    let observer = Database::from(TokioPostgresDatabase::from_url(&database_url(), 1).unwrap());
    let backend_pid = |database: Database| async move {
        database
            .fetch_one("select pg_backend_pid() as pid", DbParams::new())
            .await
            .unwrap()
            .try_get::<i32>("pid")
            .unwrap()
    };
    let original_pid = backend_pid(database.clone()).await;

    // Send BEGIN but do not poll its response. A separate connection confirms
    // that PostgreSQL opened the transaction before the caller cancels.
    let mut begin = Box::pin(database.begin());
    assert!(futures::poll!(begin.as_mut()).is_pending());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let open = observer.fetch_one(
                "select exists(select 1 from pg_stat_activity where pid = $1 and state = 'idle in transaction') as open",
                vec![DbValue::I32(original_pid)].into(),
            ).await.unwrap().try_get::<bool>("open").unwrap();
            if open {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("BEGIN did not reach PostgreSQL");
    drop(begin);

    let replacement_pid = tokio::time::timeout(Duration::from_secs(5), backend_pid(database))
        .await
        .expect("pool did not replace the discarded connection");
    assert_ne!(
        replacement_pid, original_pid,
        "the pool reused a connection with an unconfirmed transaction"
    );
}

#[cfg(feature = "driver-tokio-postgres")]
#[tokio::test]
async fn tokio_postgres_listener_reconnects_after_connection_loss() {
    exercise_listener_reconnect(false).await;
}

#[cfg(feature = "driver-tokio-postgres")]
#[tokio::test]
async fn tokio_postgres_listener_resubscribes_to_every_channel_after_connection_loss() {
    exercise_listener_reconnect(true).await;
}

#[cfg(feature = "driver-tokio-postgres")]
async fn exercise_listener_reconnect(multiple_channels: bool) {
    let channel = unique_channel("database_driver_tokio_reconnect");
    let additional = unique_channel("database_driver_tokio_second");
    let channels: &[&str] = if multiple_channels {
        &[&additional]
    } else {
        &[]
    };
    let application_name = unique_channel("database_driver_tokio_listener");
    let listen_query = std::iter::once(channel.as_str())
        .chain(channels.iter().copied())
        .map(|channel| {
            format!(
                "LISTEN {}",
                graphile_worker_database::escape_identifier(channel)
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let mut config = database_url().parse::<tokio_postgres::Config>().unwrap();
    config.application_name(&application_name);
    let tokio_database = TokioPostgresDatabase::from_config(config, 4).unwrap();
    let database = Database::from(tokio_database);

    let mut stream = if multiple_channels {
        database.listen_with_channels(&channel, channels).await
    } else {
        database.listen(&channel).await
    }
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
    if multiple_channels {
        notify(&database, &additional, "second-channel").await;
        expect_notification(&mut stream, &additional, "second-channel").await;
    }
}

/// Verifies the opt-out against server prepared statements in both query paths.
#[cfg(feature = "driver-sqlx")]
#[tokio::test]
async fn sqlx_can_disable_persistent_statements_including_transactions() {
    use ::sqlx::Row;
    for enabled in [true, false] {
        let pool = ::sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url())
            .await
            .unwrap();
        let database =
            Database::from(SqlxDatabase::new(pool.clone()).with_prepared_statements(enabled));
        let sql = "select $1::int as upstream_prepared_statement_test";
        database
            .execute(sql, vec![DbValue::I32(1)].into())
            .await
            .unwrap();
        let row = database
            .fetch_one(sql, vec![DbValue::I32(2)].into())
            .await
            .unwrap();
        assert_eq!(
            row.try_get::<i32>("upstream_prepared_statement_test")
                .unwrap(),
            2
        );
        let tx = database.begin().await.unwrap();
        let tx_sql = "select $1::int as upstream_transaction_statement_test";
        tx.execute(tx_sql, vec![DbValue::I32(3)].into())
            .await
            .unwrap();
        let row = tx
            .fetch_one(tx_sql, vec![DbValue::I32(4)].into())
            .await
            .unwrap();
        assert_eq!(
            row.try_get::<i32>("upstream_transaction_statement_test")
                .unwrap(),
            4
        );
        tx.commit().await.unwrap();
        let rows = ::sqlx::raw_sql("select count(*)::int as count from pg_prepared_statements where statement like 'select $1::int as upstream_%_test'")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(rows[0].get::<i32, _>("count"), if enabled { 2 } else { 0 });
        pool.close().await;
    }
}

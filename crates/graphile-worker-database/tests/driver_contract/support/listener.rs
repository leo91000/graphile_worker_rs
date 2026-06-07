use super::super::*;

pub(crate) async fn exercise_listen(database: &Database, channel: &str) {
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

#[cfg(feature = "driver-tokio-postgres")]
pub(crate) async fn wait_for_tokio_postgres_listener_pid(
    database: &Database,
    application_name: &str,
    listen_query: &str,
    excluded_pid: Option<i32>,
) -> i32 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    loop {
        let row = database
            .fetch_optional(
                r#"
                    select pid::int4 as pid
                    from pg_stat_activity
                    where application_name = $1
                      and query = $2
                      and ($3::int4 is null or pid <> $3)
                    order by backend_start desc
                    limit 1
                "#,
                DbParams::from(vec![
                    DbValue::Text(application_name.to_string()),
                    DbValue::Text(listen_query.to_string()),
                    DbValue::I32Opt(excluded_pid),
                ]),
            )
            .await
            .unwrap();

        if let Some(row) = row {
            return row.try_get::<i32>("pid").unwrap();
        }

        assert!(
            tokio::time::Instant::now() < deadline,
            "listener backend should reconnect before timeout"
        );

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(feature = "driver-tokio-postgres")]
pub(crate) async fn notify(database: &Database, channel: &str, payload: &str) {
    database
        .execute(
            "select pg_notify($1, $2)",
            DbParams::from(vec![
                DbValue::Text(channel.to_string()),
                DbValue::Text(payload.to_string()),
            ]),
        )
        .await
        .unwrap();
}

#[cfg(feature = "driver-tokio-postgres")]
pub(crate) async fn expect_notification(
    stream: &mut NotificationStream,
    channel: &str,
    payload: &str,
) {
    let notification = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("notification should arrive")
        .expect("notification stream should stay open")
        .unwrap();

    assert_eq!(notification.channel, channel);
    assert_eq!(notification.payload, payload);
}

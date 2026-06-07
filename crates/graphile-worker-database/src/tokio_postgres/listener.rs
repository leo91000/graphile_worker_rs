use std::time::Duration;

use ::tokio_postgres::{AsyncMessage, Client, NoTls};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{escape_identifier, DbError, Notification, NotificationStream};

const INITIAL_LISTENER_RECONNECT_DELAY: Duration = Duration::from_millis(50);
const MAX_LISTENER_RECONNECT_DELAY: Duration = Duration::from_secs(5);

pub(super) async fn listen(
    config: Option<::tokio_postgres::Config>,
    channel: &str,
) -> Result<Option<NotificationStream>, DbError> {
    let Some(config) = config else {
        return Ok(None);
    };

    let sql = format!("LISTEN {}", escape_identifier(channel));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (client, closed_rx) = connect_listener(&config, &sql, &tx).await?;

    drop(tokio::spawn(run_reconnecting_listener(
        config, sql, tx, client, closed_rx,
    )));

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

    Ok(Some(Box::pin(stream) as NotificationStream))
}

fn next_reconnect_delay(delay: Duration) -> Duration {
    let doubled = delay.checked_mul(2).unwrap_or(MAX_LISTENER_RECONNECT_DELAY);

    if doubled > MAX_LISTENER_RECONNECT_DELAY {
        MAX_LISTENER_RECONNECT_DELAY
    } else {
        doubled
    }
}

async fn connect_listener(
    config: &::tokio_postgres::Config,
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

async fn run_reconnecting_listener(
    config: ::tokio_postgres::Config,
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

            match connect_listener(&config, &listen_sql, &tx).await {
                Ok((new_client, new_closed_rx)) => {
                    client = new_client;
                    closed_rx = new_closed_rx;
                    reconnect_delay = INITIAL_LISTENER_RECONNECT_DELAY;
                    break;
                }
                Err(_) => {
                    reconnect_delay = next_reconnect_delay(reconnect_delay);
                }
            }
        }
    }
}

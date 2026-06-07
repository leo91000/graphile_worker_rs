use std::pin::Pin;

use futures::Stream;

use crate::DbError;

pub type NotificationStream = Pin<Box<dyn Stream<Item = Result<Notification, DbError>> + Send>>;

#[derive(Clone, Debug)]
pub struct Notification {
    pub channel: String,
    pub payload: String,
}

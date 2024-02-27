use serde::{Deserialize, Serialize};

use crate::handler::TaskHandler;

pub trait TaskDefinition<T> {
    type Payload: for<'de> Deserialize<'de> + Serialize + Send + 'static;
    fn get_task_runner(&self) -> impl TaskHandler<T> + Clone + 'static;
    fn identifier() -> &'static str;
}

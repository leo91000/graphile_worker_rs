use serde::{Deserialize, Serialize};

use crate::handler::TaskHandler;

pub trait TaskDefinition<Context>
where
    Context: Send,
{
    type Payload: for<'de> Deserialize<'de> + Serialize + Send + 'static;
    fn get_task_runner(&self) -> impl TaskHandler<Self::Payload, Context> + Clone + 'static;
    fn identifier() -> &'static str;
}

use std::{collections::VecDeque, ops::Add, rc::Rc};

use crate::helpers::StaticCounter;
use time::{Duration, OffsetDateTime};
use graphile_worker::{IntoTaskHandlerResult, JobSpec, JobSpecBuilder, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    sync::{oneshot, Mutex, OnceCell},
    task::spawn_local,
    time::{sleep, Instant},
};

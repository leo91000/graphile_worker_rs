use super::*;

#[path = "local_queue_hooks/get_jobs_complete.rs"]
mod get_jobs_complete;
#[path = "local_queue_hooks/init.rs"]
mod init;
#[path = "local_queue_hooks/refetch_delay.rs"]
mod refetch_delay;
#[path = "local_queue_hooks/return_jobs.rs"]
mod return_jobs;
#[path = "local_queue_hooks/set_mode.rs"]
mod set_mode;

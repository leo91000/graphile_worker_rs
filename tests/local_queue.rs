#![allow(unused_imports)]

#[path = "helpers/mod.rs"]
mod helpers;

include!("local_queue/common.rs");
#[path = "local_queue/basic.rs"]
mod basic;
#[path = "local_queue/distribution.rs"]
mod distribution;
#[path = "local_queue/multi_queue.rs"]
mod multi_queue;
#[path = "local_queue/refetch.rs"]
mod refetch;
#[path = "local_queue/shutdown_ttl.rs"]
mod shutdown_ttl;

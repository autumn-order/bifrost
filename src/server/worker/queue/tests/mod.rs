use crate::server::worker::{queue::config::WorkerQueueConfig, WorkerJobQueue};

pub mod cleanup;
pub mod pop;
pub mod push;
pub mod schedule;

use bifrost_test_utils::prelude::*;

pub fn setup_test_queue(redis: &RedisTest) -> WorkerJobQueue {
    let config = WorkerQueueConfig {
        queue_name: redis.queue_name(),
        job_ttl: std::time::Duration::from_secs(3600),
        cleanup_interval: std::time::Duration::from_millis(100),
    };

    WorkerJobQueue::with_config(redis.redis_pool.clone(), config)
}

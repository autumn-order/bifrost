mod config;
mod lifecycle;
mod pool;

use bifrost_test_utils::prelude::*;

use crate::server::worker::{queue::config::WorkerQueueConfig, WorkerJobQueue};

pub fn setup_test_queue(redis: &RedisTest) -> WorkerJobQueue {
    let config = WorkerQueueConfig {
        queue_name: redis.queue_name(),
        job_ttl: std::time::Duration::from_secs(5),
        cleanup_interval: std::time::Duration::from_millis(50),
    };

    WorkerJobQueue::with_config(redis.redis_pool.clone(), config)
}

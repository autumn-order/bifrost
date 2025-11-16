mod lifecycle;
mod pool;

use bifrost::server::worker::{queue::config::WorkerQueueConfig, WorkerQueue};

use crate::redis::RedisTest;

pub fn setup_test_queue(redis: &RedisTest) -> WorkerQueue {
    let config = WorkerQueueConfig {
        queue_name: redis.queue_name(),
        job_ttl: std::time::Duration::from_secs(5),
        cleanup_interval: std::time::Duration::from_millis(50),
    };

    WorkerQueue::with_config(redis.redis_pool.clone(), config)
}

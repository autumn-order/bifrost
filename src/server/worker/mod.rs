pub mod handler;
pub mod pool;
pub mod queue;

use fred::prelude::Pool;
pub use pool::WorkerPool;
pub use queue::WorkerQueue;

use crate::server::worker::{handler::WorkerJobHandler, pool::WorkerPoolConfig};

#[derive(Clone)]
pub struct Worker {
    pub queue: WorkerQueue,
    pub pool: WorkerPool,
}

impl Worker {
    pub fn new(max_concurrent_jobs: usize, redis_pool: Pool, handler: WorkerJobHandler) -> Self {
        let config = WorkerPoolConfig::new(max_concurrent_jobs);
        let queue = WorkerQueue::new(redis_pool);
        let pool = WorkerPool::new(config, queue.clone(), handler);

        Self { queue, pool }
    }
}

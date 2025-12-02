//! Background worker system for asynchronous job processing.
//!
//! This module provides a Redis-backed job queue and worker pool for processing
//! background tasks asynchronously. Jobs are scheduled with deduplication, TTL-based
//! cleanup, and configurable concurrency limits. The system handles EVE Online data
//! updates including faction, alliance, corporation, character info, and affiliations.

pub mod handler;
pub mod pool;
pub mod queue;

use fred::prelude::Pool;
pub use pool::WorkerPool;
pub use queue::WorkerQueue;

use crate::server::worker::{handler::WorkerJobHandler, pool::WorkerPoolConfig};

/// Combined worker system with queue and processing pool.
///
/// Provides a convenient wrapper that combines the job queue (Redis-backed) and
/// worker pool (processing tasks) into a single interface for managing background jobs.
#[derive(Clone)]
pub struct Worker {
    pub queue: WorkerQueue,
    pub pool: WorkerPool,
}

impl Worker {
    /// Creates a new worker system with queue and pool.
    ///
    /// Initializes both the job queue backed by Redis and the worker pool for processing
    /// jobs with the specified concurrency limit. The queue handles job scheduling and
    /// deduplication while the pool manages job execution.
    ///
    /// # Arguments
    /// - `max_concurrent_jobs` - Maximum number of jobs that can be processed simultaneously
    /// - `redis_pool` - Redis connection pool for job queue storage
    /// - `handler` - Job handler that processes different job types
    ///
    /// # Returns
    /// - `Worker` - New worker system ready to start processing jobs
    pub fn new(max_concurrent_jobs: usize, redis_pool: Pool, handler: WorkerJobHandler) -> Self {
        let config = WorkerPoolConfig::new(max_concurrent_jobs);
        let queue = WorkerQueue::new(redis_pool);
        let pool = WorkerPool::new(config, queue.clone(), handler);

        Self { queue, pool }
    }
}

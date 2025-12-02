//! Worker queue configuration for TTL and cleanup settings.
//!
//! This module provides the `WorkerQueueConfig` struct for configuring job queue
//! behavior including queue naming, job TTL (time-to-live), and cleanup intervals.
//! Jobs exceeding the TTL are automatically removed during cleanup operations.

use std::time::Duration;

const DEFAULT_QUEUE_NAME: &str = "bifrost:worker:queue";

/// Maximum age for jobs in the queue before they're considered stale (1 hour in seconds)
/// Jobs older than this will be removed by cleanup operations
const DEFAULT_JOB_TTL: Duration = Duration::from_secs(3600);

/// Cleanup interval in milliseconds (5 minutes in seconds)
/// Cleanup will run at most once per this interval
const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

/// Configuration for the worker queue.
///
/// Defines queue naming, job TTL for stale job removal, and cleanup task interval.
/// Provides sensible defaults optimized for production use with 1-hour job TTL and
/// 5-minute cleanup intervals.
#[derive(Clone)]
pub struct WorkerQueueConfig {
    /// Redis key name for the job queue sorted set
    pub queue_name: String,
    /// Maximum age for jobs before considered stale and removed by cleanup
    pub job_ttl: Duration,
    /// How often the cleanup task runs to remove stale jobs
    pub cleanup_interval: Duration,
}

impl WorkerQueueConfig {
    /// Creates a new queue configuration with default values.
    ///
    /// Initializes configuration with default queue name, 1-hour job TTL, and
    /// 5-minute cleanup interval.
    ///
    /// # Returns
    /// - `WorkerQueueConfig` - New configuration with default values
    fn new() -> Self {
        Self {
            queue_name: DEFAULT_QUEUE_NAME.to_string(),
            job_ttl: DEFAULT_JOB_TTL,
            cleanup_interval: DEFAULT_CLEANUP_INTERVAL,
        }
    }
}

impl Default for WorkerQueueConfig {
    fn default() -> Self {
        Self::new()
    }
}

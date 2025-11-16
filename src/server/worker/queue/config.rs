use std::time::Duration;

const DEFAULT_QUEUE_NAME: &str = "bifrost:worker:queue";

/// Maximum age for jobs in the queue before they're considered stale (1 hour in seconds)
/// Jobs older than this will be removed by cleanup operations
const DEFAULT_JOB_TTL: Duration = Duration::from_secs(3600);

/// Cleanup interval in milliseconds (5 minutes in seconds)
/// Cleanup will run at most once per this interval
const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct WorkerQueueConfig {
    pub queue_name: String,
    pub job_ttl: Duration,
    pub cleanup_interval: Duration,
}

impl WorkerQueueConfig {
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

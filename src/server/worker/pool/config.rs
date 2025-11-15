use std::time::Duration;

/// Configuration for the worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Maximum concurrent jobs that can be processed simultaneously.
    ///
    /// Set this to ~80% of your PostgreSQL connection pool size to avoid
    /// connection exhaustion. For example, if your DB pool has 100 connections,
    /// set this to 80.
    pub max_concurrent_jobs: usize,

    /// Number of dispatcher tasks that poll Redis for jobs.
    ///
    /// 1-2 dispatchers work well for most workloads. More than 3 provides
    /// diminishing returns.
    pub dispatcher_count: usize,

    /// How long to wait between polls when the queue is empty (milliseconds).
    pub poll_interval_ms: u64,

    /// Maximum time a job can run before being cancelled (seconds).
    pub job_timeout_seconds: u64,
}

impl WorkerPoolConfig {
    /// Create a new configuration with sensible defaults
    ///
    /// # Arguments
    /// * `max_concurrent_jobs` - Maximum concurrent jobs (~80% of DB pool size)
    pub fn new(max_concurrent_jobs: usize) -> Self {
        Self {
            max_concurrent_jobs,
            dispatcher_count: 2, // Good default for most workloads
            poll_interval_ms: 50,
            job_timeout_seconds: 300, // 5 minutes
        }
    }

    /// Get job timeout as Duration
    pub fn job_timeout(&self) -> Duration {
        Duration::from_secs(self.job_timeout_seconds)
    }

    /// Get poll interval as Duration
    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_interval_ms)
    }
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self::new(50)
    }
}

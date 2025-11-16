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
    /// Automatically calculated as 1 dispatcher per 40 concurrent jobs (minimum 1).
    /// This ensures adequate polling capacity as concurrency scales.
    pub dispatcher_count: usize,

    /// How long to wait between polls when the queue is empty (milliseconds).
    pub poll_interval_ms: u64,

    /// Maximum time a job can run before being cancelled (seconds).
    pub job_timeout_seconds: u64,

    /// Maximum time to wait for a dispatcher to shutdown (seconds).
    /// If a dispatcher doesn't stop within this time, a warning is logged.
    pub shutdown_timeout_seconds: u64,

    /// How often the queue cleanup task runs to remove stale jobs (milliseconds).
    /// The cleanup task removes jobs older than the TTL.
    pub cleanup_interval_ms: u64,
}

impl WorkerPoolConfig {
    /// Create a new configuration with sensible defaults
    ///
    /// # Arguments
    /// * `max_concurrent_jobs` - Maximum concurrent jobs (~80% of DB pool size)
    pub fn new(max_concurrent_jobs: usize) -> Self {
        // Scale dispatchers: 1 per 40 concurrent jobs, minimum 1
        // Use ceiling division to ensure no more than 40 jobs per dispatcher
        let dispatcher_count = ((max_concurrent_jobs + 39) / 40).max(1);

        Self {
            max_concurrent_jobs,
            dispatcher_count,
            poll_interval_ms: 50,    // 50ms between polls when queue is empty
            job_timeout_seconds: 60, // 1 minute
            shutdown_timeout_seconds: 5, // 5 seconds to wait for dispatcher shutdown
            cleanup_interval_ms: 5 * 60 * 1000, // 5 minutes
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

    /// Get shutdown timeout as Duration
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_seconds)
    }

    /// Get cleanup interval as Duration
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_millis(self.cleanup_interval_ms)
    }
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self::new(4)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::server::worker::pool::WorkerPoolConfig;

    #[test]
    fn test_default_config() {
        let config = WorkerPoolConfig::default();

        assert_eq!(
            config.max_concurrent_jobs, 4,
            "Default max_concurrent_jobs should be 4"
        );
        assert_eq!(
            config.dispatcher_count, 1,
            "Default dispatcher_count should be 1 (4 / 40 = 1, min 1)"
        );
        assert_eq!(
            config.poll_interval_ms, 50,
            "Default poll_interval_ms should be 50"
        );
        assert_eq!(
            config.job_timeout_seconds, 60,
            "Default job_timeout_seconds should be 60 (1 minute)"
        );
        assert_eq!(
            config.shutdown_timeout_seconds, 5,
            "Default shutdown_timeout_seconds should be 5"
        );
        assert_eq!(
            config.cleanup_interval_ms,
            5 * 60 * 1000,
            "Default cleanup_interval_ms should be 300000 (5 minutes)"
        );
    }

    #[test]
    fn test_new_config_with_custom_max_concurrent_jobs() {
        let config = WorkerPoolConfig::new(100);

        assert_eq!(
            config.max_concurrent_jobs, 100,
            "max_concurrent_jobs should be 100"
        );
        assert_eq!(
            config.dispatcher_count, 3,
            "dispatcher_count should be 3 (100 jobs needs 3 dispatchers)"
        );
        assert_eq!(
            config.poll_interval_ms, 50,
            "poll_interval_ms should be default 50"
        );
        assert_eq!(
            config.job_timeout_seconds, 60,
            "job_timeout_seconds should be default 60"
        );
        assert_eq!(
            config.shutdown_timeout_seconds, 5,
            "shutdown_timeout_seconds should be default 5"
        );
        assert_eq!(
            config.cleanup_interval_ms,
            5 * 60 * 1000,
            "cleanup_interval_ms should be default 300000"
        );
    }

    #[test]
    fn test_job_timeout_conversion() {
        let mut config = WorkerPoolConfig::new(50);
        config.job_timeout_seconds = 120;

        let timeout = config.job_timeout();
        assert_eq!(
            timeout,
            Duration::from_secs(120),
            "job_timeout() should return Duration from seconds"
        );
    }

    #[test]
    fn test_poll_interval_conversion() {
        let mut config = WorkerPoolConfig::new(50);
        config.poll_interval_ms = 100;

        let interval = config.poll_interval();
        assert_eq!(
            interval,
            Duration::from_millis(100),
            "poll_interval() should return Duration from milliseconds"
        );
    }

    #[test]
    fn test_shutdown_timeout_conversion() {
        let mut config = WorkerPoolConfig::new(50);
        config.shutdown_timeout_seconds = 10;

        let timeout = config.shutdown_timeout();
        assert_eq!(
            timeout,
            Duration::from_secs(10),
            "shutdown_timeout() should return Duration from seconds"
        );
    }

    #[test]
    fn test_cleanup_interval_conversion() {
        let mut config = WorkerPoolConfig::new(50);
        config.cleanup_interval_ms = 60000;

        let interval = config.cleanup_interval();
        assert_eq!(
            interval,
            Duration::from_millis(60000),
            "cleanup_interval() should return Duration from milliseconds"
        );
    }

    #[test]
    fn test_config_clone() {
        let config1 = WorkerPoolConfig::new(80);
        let config2 = config1.clone();

        assert_eq!(
            config1.max_concurrent_jobs, config2.max_concurrent_jobs,
            "Cloned config should have same max_concurrent_jobs"
        );
        assert_eq!(
            config1.dispatcher_count, config2.dispatcher_count,
            "Cloned config should have same dispatcher_count"
        );
    }

    #[test]
    fn test_config_with_custom_timeouts() {
        let mut config = WorkerPoolConfig::new(50);
        config.job_timeout_seconds = 10;
        config.shutdown_timeout_seconds = 3;
        config.cleanup_interval_ms = 30000;

        assert_eq!(config.job_timeout(), Duration::from_secs(10));
        assert_eq!(config.shutdown_timeout(), Duration::from_secs(3));
        assert_eq!(config.cleanup_interval(), Duration::from_millis(30000));
    }

    #[test]
    fn test_realistic_production_config() {
        let config = WorkerPoolConfig::new(4);

        assert_eq!(config.max_concurrent_jobs, 4);
        assert_eq!(config.dispatcher_count, 1);
        assert_eq!(config.job_timeout(), Duration::from_secs(60));
        assert_eq!(config.shutdown_timeout(), Duration::from_secs(5));
        assert_eq!(
            config.cleanup_interval(),
            Duration::from_millis(5 * 60 * 1000)
        );
    }

    #[test]
    fn test_short_timeouts_for_testing() {
        let mut config = WorkerPoolConfig::new(10);
        config.poll_interval_ms = 10;
        config.job_timeout_seconds = 1;
        config.shutdown_timeout_seconds = 1;
        config.cleanup_interval_ms = 100;

        assert_eq!(config.poll_interval(), Duration::from_millis(10));
        assert_eq!(config.job_timeout(), Duration::from_secs(1));
        assert_eq!(config.shutdown_timeout(), Duration::from_secs(1));
        assert_eq!(config.cleanup_interval(), Duration::from_millis(100));
    }

    #[test]
    fn test_dispatcher_scaling() {
        // Test various scaling scenarios with ceiling division
        // Formula: (max_concurrent_jobs + 39) / 40
        assert_eq!(
            WorkerPoolConfig::new(1).dispatcher_count,
            1,
            "1 job should have 1 dispatcher"
        );
        assert_eq!(
            WorkerPoolConfig::new(39).dispatcher_count,
            1,
            "39 jobs should have 1 dispatcher"
        );
        assert_eq!(
            WorkerPoolConfig::new(40).dispatcher_count,
            1,
            "40 jobs should have 1 dispatcher (max for 1 dispatcher)"
        );
        assert_eq!(
            WorkerPoolConfig::new(41).dispatcher_count,
            2,
            "41 jobs should have 2 dispatchers"
        );
        assert_eq!(
            WorkerPoolConfig::new(80).dispatcher_count,
            2,
            "80 jobs should have 2 dispatchers (max for 2 dispatchers)"
        );
        assert_eq!(
            WorkerPoolConfig::new(81).dispatcher_count,
            3,
            "81 jobs should have 3 dispatchers"
        );
        assert_eq!(
            WorkerPoolConfig::new(119).dispatcher_count,
            3,
            "119 jobs should have 3 dispatchers"
        );
        assert_eq!(
            WorkerPoolConfig::new(120).dispatcher_count,
            3,
            "120 jobs should have 3 dispatchers (max for 3 dispatchers)"
        );
        assert_eq!(
            WorkerPoolConfig::new(121).dispatcher_count,
            4,
            "121 jobs should have 4 dispatchers"
        );
    }
}

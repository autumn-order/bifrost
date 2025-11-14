use dioxus_logger::tracing;

/// Configuration for the worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Maximum concurrent jobs. Set to ~80% of PostgreSQL connection pool size.
    pub max_concurrent_jobs: usize,

    /// Base delay between polls when queue is empty (milliseconds).
    /// Jittered by up to 50% to prevent thundering herd.
    pub poll_interval_ms: u64,

    /// Delay between spawning each dispatcher (milliseconds).
    pub dispatcher_spawn_stagger_ms: u64,

    /// Maximum initial random delay before dispatcher starts (milliseconds).
    pub dispatcher_initial_jitter_ms: u64,

    /// Maximum consecutive errors before backing off.
    pub max_consecutive_errors: u32,

    /// Backoff duration after max consecutive errors (seconds).
    pub error_backoff_seconds: u64,

    /// Maximum job execution time before cancellation (seconds).
    pub job_timeout_seconds: u64,
}

impl WorkerPoolConfig {
    /// Create configuration with automatic scaling.
    ///
    /// **PostgreSQL Connection Bottleneck**
    /// Each job requires a DB connection for read/write operations. This is the primary
    /// bottleneck. Set `max_concurrent_jobs` to ~80% of your PostgreSQL connection pool
    /// size (e.g., pool of 100 → max_concurrent_jobs = 80).
    ///
    /// **Dispatcher Autoscaling**
    /// Dispatchers poll Redis and fetch jobs in batches (10-50 jobs/call). They're
    /// lightweight and scale with workload: 1 dispatcher per 100 concurrent jobs.
    ///
    /// # Arguments
    /// * `max_concurrent_jobs` - Max concurrent jobs (~80% of PostgreSQL pool size)
    pub fn new(max_concurrent_jobs: usize) -> Self {
        // Most PostgreSQL deployments won't support >500 worker connections
        if max_concurrent_jobs > 500 {
            tracing::warn!(
                "max_concurrent_jobs ({}) exceeds typical PostgreSQL limits. \
                  Ensure your connection pool can handle this load.",
                max_concurrent_jobs
            );
        }

        Self {
            max_concurrent_jobs,
            poll_interval_ms: Self::calculate_poll_interval(max_concurrent_jobs),
            dispatcher_spawn_stagger_ms: 150,
            dispatcher_initial_jitter_ms: 500,
            max_consecutive_errors: 5,
            error_backoff_seconds: 10,
            job_timeout_seconds: 300,
        }
    }

    /// Calculate dispatcher count based on concurrent jobs and burst handling needs.
    ///
    /// Dispatchers are lightweight and primarily useful for:
    /// 1. Burst handling when many jobs arrive simultaneously
    /// 2. Redundancy if one dispatcher has Redis latency spike
    /// 3. Continued operation if one dispatcher panics
    ///
    /// With batch fetching, a single dispatcher can handle 300-400 jobs/sec.
    /// Multiple dispatchers improve burst response and fault tolerance.
    pub fn dispatcher_count(&self) -> usize {
        match self.max_concurrent_jobs {
            0..=150 => 1,   // Single dispatcher sufficient for low-medium load
            151..=300 => 2, // Two for redundancy and burst handling
            _ => 3,         // Three dispatchers max - more doesn't help
        }
    }

    /// Calculate prefetch batch size based on concurrent job capacity.
    ///
    /// Batch size balances:
    /// - **Larger batches**: Fewer Redis round-trips, better throughput
    /// - **Smaller batches**: Less memory buffering, more even dispatcher load
    ///
    /// With multiple dispatchers, each independently fetches its batch,
    /// so total buffering = batch_size * dispatcher_count.
    pub fn prefetch_batch_size(&self) -> usize {
        match self.max_concurrent_jobs {
            0..=50 => 20,   // Smaller batches to reduce buffering overhead
            51..=150 => 30, // Sweet spot for most workloads
            _ => 40,        // Larger batches for high-throughput scenarios
        }
    }

    /// Calculate poll interval based on concurrent jobs (lower = more responsive).
    fn calculate_poll_interval(max_concurrent_jobs: usize) -> u64 {
        match max_concurrent_jobs {
            0..=10 => 100,
            11..=25 => 75,
            26..=50 => 60,
            _ => 50,
        }
    }

    pub fn job_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.job_timeout_seconds)
    }

    pub fn dispatcher_spawn_stagger(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.dispatcher_spawn_stagger_ms)
    }

    pub fn dispatcher_initial_jitter(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.dispatcher_initial_jitter_ms)
    }
}

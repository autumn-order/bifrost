/// Configuration for the worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Maximum number of concurrent jobs that can be processed simultaneously
    ///
    /// This is enforced via a semaphore. Actual number of active tasks will be
    /// between 0 and max_concurrent_jobs depending on workload.
    pub max_concurrent_jobs: usize,

    /// Base delay between polling attempts when queue is empty (in milliseconds)
    ///
    /// Actual delay will be `poll_interval_ms + random(0..poll_interval_ms/2)` to prevent
    /// thundering herd when multiple dispatchers wake simultaneously.
    pub poll_interval_ms: u64,

    /// Delay between spawning each dispatcher (in milliseconds)
    ///
    /// Staggering dispatcher spawns prevents all dispatchers from hitting Redis
    /// simultaneously on startup, reducing initial load spike.
    pub dispatcher_spawn_stagger_ms: u64,

    /// Maximum initial random delay before a dispatcher starts polling (in milliseconds)
    ///
    /// Each dispatcher will sleep for a random duration between 0 and this value
    /// before beginning its main loop, further distributing initial load.
    pub dispatcher_initial_jitter_ms: u64,

    /// Maximum number of consecutive errors before a dispatcher backs off
    pub max_consecutive_errors: u32,

    /// Backoff duration after max consecutive errors (in seconds)
    pub error_backoff_seconds: u64,

    /// Maximum time a job can run before being cancelled (in seconds)
    ///
    /// If a job exceeds this timeout, it will be cancelled and the semaphore permit
    /// will be released. This prevents hung jobs from holding permits indefinitely.
    pub job_timeout_seconds: u64,
}

impl WorkerPoolConfig {
    /// Create a new WorkerPoolConfig with automatic scaling
    ///
    /// Automatically calculates optimal values for:
    /// - `dispatcher_count`: 1 dispatcher per 25-50 jobs (min 1, max 5)
    /// - `prefetch_batch_size`: Scaled based on concurrent job limit
    /// - `poll_interval_ms`: Adjusted for concurrency (more aggressive for high concurrency)
    ///
    /// # Arguments
    /// * `max_concurrent_jobs` - Maximum number of jobs that can run concurrently
    ///
    /// # Usage
    /// Small scale (10 concurrent jobs, 1 dispatcher)
    /// Medium scale (50 concurrent jobs, 2 dispatchers)
    /// High scale (200 concurrent jobs, 5 dispatchers)
    pub fn new(max_concurrent_jobs: usize) -> Self {
        Self {
            max_concurrent_jobs,
            poll_interval_ms: Self::calculate_poll_interval(max_concurrent_jobs),
            dispatcher_spawn_stagger_ms: 150, // 150ms between spawning each dispatcher
            dispatcher_initial_jitter_ms: 500, // Up to 500ms random initial delay per dispatcher
            max_consecutive_errors: 5,
            error_backoff_seconds: 10,
            job_timeout_seconds: 300, // 5 minutes default timeout
        }
    }

    /// Calculate optimal number of dispatcher threads based on max concurrent jobs
    ///
    /// Rule: 1 dispatcher per 25-50 concurrent jobs, min 1, max 5
    pub fn dispatcher_count(&self) -> usize {
        let count = (self.max_concurrent_jobs as f64 / 40.0).ceil() as usize;
        count.clamp(1, 5)
    }

    /// Calculate optimal prefetch batch size based on concurrent jobs and dispatchers
    ///
    /// Rule: Aim for each dispatcher to fetch enough jobs to keep workers busy
    /// without overwhelming the semaphore queue
    pub fn prefetch_batch_size(&self) -> usize {
        let dispatchers = self.dispatcher_count();
        let batch = (self.max_concurrent_jobs as f64 / (dispatchers as f64 * 5.0)).ceil() as usize;
        batch.clamp(5, 25)
    }

    /// Calculate optimal poll interval based on max concurrent jobs
    ///
    /// Higher concurrency gets more aggressive polling for lower latency
    fn calculate_poll_interval(max_concurrent_jobs: usize) -> u64 {
        match max_concurrent_jobs {
            0..=10 => 100, // Low concurrency: 100ms
            11..=25 => 75, // Medium concurrency: 75ms
            26..=50 => 60, // Medium-high concurrency: 60ms
            _ => 50,       // High concurrency: 50ms
        }
    }

    /// Get the configured job timeout duration
    pub fn job_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.job_timeout_seconds)
    }

    /// Get the configured dispatcher spawn stagger duration
    pub fn dispatcher_spawn_stagger(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.dispatcher_spawn_stagger_ms)
    }

    /// Get the configured dispatcher initial jitter duration
    pub fn dispatcher_initial_jitter(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.dispatcher_initial_jitter_ms)
    }
}

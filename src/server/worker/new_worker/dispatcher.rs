use std::sync::Arc;
use std::time::Duration;

use dioxus_logger::tracing;
use rand::Rng;
use sea_orm::DatabaseConnection;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use crate::server::worker::queue::WorkerJobQueue;

use super::{config::WorkerPoolConfig, handler::WorkerJobHandler};

/// Handle for a dispatcher task
pub(super) struct DispatcherHandle {
    pub id: usize,
    pub handle: JoinHandle<()>,
}

impl DispatcherHandle {
    /// Spawn a new dispatcher task
    ///
    /// Dispatchers continuously poll Redis for jobs and spawn tasks to process them.
    /// A semaphore limits the number of concurrent job-processing tasks, providing
    /// backpressure when the system is at capacity.
    pub fn spawn(
        id: usize,
        config: WorkerPoolConfig,
        queue: Arc<WorkerJobQueue>,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        semaphore: Arc<Semaphore>,
        shutdown_signal: Arc<tokio::sync::Notify>,
    ) -> Self {
        let handle = tokio::spawn(async move {
            tracing::info!("Dispatcher {} started", id);
            Self::dispatcher_loop(
                id,
                config,
                queue,
                db,
                esi_client,
                semaphore,
                shutdown_signal,
            )
            .await;
            tracing::info!("Dispatcher {} stopped", id);
        });

        Self { id, handle }
    }

    /// Main dispatcher loop
    ///
    /// Continuously polls Redis for jobs and spawns tasks to process them.
    /// Uses jittered backoff to prevent thundering herd when queue is empty.
    ///
    /// # Concurrency Control
    /// - Semaphore limits concurrent job-processing tasks
    /// - When at capacity, `acquire_owned()` blocks until a task completes
    /// - Each dispatcher tries to fetch `prefetch_batch_size` jobs per cycle
    /// - Jittered sleep prevents all dispatchers from waking simultaneously
    async fn dispatcher_loop(
        id: usize,
        config: WorkerPoolConfig,
        queue: Arc<WorkerJobQueue>,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        semaphore: Arc<Semaphore>,
        shutdown_signal: Arc<tokio::sync::Notify>,
    ) {
        let mut consecutive_errors = 0u32;
        let mut jobs_dispatched = 0u64;
        let prefetch_batch_size = config.prefetch_batch_size();

        loop {
            // Check for shutdown signal
            tokio::select! {
                _ = shutdown_signal.notified() => {
                    tracing::debug!("Dispatcher {} received shutdown signal", id);
                    break;
                }
                _ = async {
                    // Try to fetch multiple jobs in this cycle to reduce Redis calls
                    let mut fetched_in_cycle = 0;

                    for _ in 0..prefetch_batch_size {
                        match queue.pop().await {
                            Ok(Some(job)) => {
                                consecutive_errors = 0;
                                fetched_in_cycle += 1;

                                // Acquire semaphore permit (blocks if at capacity)
                                // This provides backpressure when system is overloaded
                                let permit = match semaphore.clone().acquire_owned().await {
                                    Ok(permit) => permit,
                                    Err(_) => {
                                        // Semaphore closed, likely shutdown
                                        tracing::debug!("Dispatcher {} semaphore closed", id);
                                        return;
                                    }
                                };

                                // Clone resources for the spawned task
                                let db = Arc::clone(&db);
                                let esi_client = Arc::clone(&esi_client);

                                // Spawn task to process job with timeout
                                // Tokio's work-stealing scheduler distributes tasks efficiently
                                let timeout_duration = config.job_timeout();
                                tokio::spawn(async move {
                                    let handler = WorkerJobHandler::new(&db, &esi_client);

                                    // Wrap job execution with timeout to prevent hung jobs
                                    let result = tokio::time::timeout(
                                        timeout_duration,
                                        handler.handle(&job)
                                    ).await;

                                    match result {
                                        Ok(Ok(())) => {
                                            // Job completed successfully
                                        }
                                        Ok(Err(e)) => {
                                            tracing::error!("Failed to process job {:?}: {:?}", job, e);
                                        }
                                        Err(_) => {
                                            tracing::error!(
                                                "Job timed out after {} seconds: {:?}",
                                                timeout_duration.as_secs(),
                                                job
                                            );
                                        }
                                    }

                                    // Permit automatically dropped here, releasing semaphore slot
                                    drop(permit);
                                });

                                jobs_dispatched += 1;
                            }
                            Ok(None) => {
                                // Queue is empty, stop trying to fetch more in this cycle
                                break;
                            }
                            Err(e) => {
                                consecutive_errors += 1;
                                tracing::error!(
                                    "Dispatcher {} error getting job from queue (consecutive errors: {}): {:?}",
                                    id,
                                    consecutive_errors,
                                    e
                                );
                                break;
                            }
                        }
                    }

                    // Log stats periodically
                    if jobs_dispatched > 0 && jobs_dispatched % 100 == 0 {
                        tracing::debug!(
                            "Dispatcher {} has dispatched {} jobs ({} permits available)",
                            id,
                            jobs_dispatched,
                            semaphore.available_permits()
                        );
                    }

                    // Determine sleep duration based on fetch results
                    if fetched_in_cycle == 0 {
                        // No jobs found or error occurred
                        if consecutive_errors >= config.max_consecutive_errors {
                            tracing::warn!(
                                "Dispatcher {} backing off for {} seconds due to {} consecutive errors",
                                id,
                                config.error_backoff_seconds,
                                consecutive_errors
                            );
                            tokio::time::sleep(Duration::from_secs(config.error_backoff_seconds)).await;
                            consecutive_errors = 0;
                        } else {
                            // Queue empty or single error, normal backoff with jitter
                            let jitter = rand::rng().random_range(0..config.poll_interval_ms / 2);
                            let delay = config.poll_interval_ms + jitter;
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                    } else if fetched_in_cycle < prefetch_batch_size {
                        // Fetched some jobs but not a full batch, queue may be draining
                        // Short sleep before next poll
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    // If we fetched a full batch, immediately try again (no sleep)
                } => {}
            }
        }
    }
}

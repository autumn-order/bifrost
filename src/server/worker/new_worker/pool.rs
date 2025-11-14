use std::sync::Arc;

use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;
use tokio::sync::{RwLock, Semaphore};

use crate::server::{error::Error, worker::queue::WorkerJobQueue};

use super::{config::WorkerPoolConfig, context::DispatcherContext, dispatcher::DispatcherHandle};

/// Worker pool for processing jobs from the WorkerJobQueue
///
/// Uses Tokio's work-stealing scheduler to efficiently distribute job processing
/// across available threads. Dispatchers poll Redis and spawn tasks directly,
/// with a semaphore controlling maximum concurrency.
pub struct WorkerPool {
    config: WorkerPoolConfig,
    context: DispatcherContext,
    dispatchers: Arc<RwLock<Vec<DispatcherHandle>>>,
}

impl WorkerPool {
    /// Create a new worker pool
    ///
    /// # Arguments
    /// - `config`: Configuration including max concurrent jobs and dispatcher settings
    /// - `db`: Database connection pool (shared across all job tasks)
    /// - `esi_client`: EVE ESI API client (shared across all job tasks)
    /// - `queue`: Redis-backed job queue
    pub fn new(
        config: WorkerPoolConfig,
        db: Arc<DatabaseConnection>,
        esi_client: Arc<eve_esi::Client>,
        queue: Arc<WorkerJobQueue>,
    ) -> Self {
        // Create semaphore to limit concurrent job processing
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_jobs));
        let shutdown_signal = Arc::new(tokio::sync::Notify::new());

        // Bundle shared resources into context
        let context = DispatcherContext::new(queue, db, esi_client, semaphore, shutdown_signal);

        Self {
            config,
            context,
            dispatchers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start the worker pool
    ///
    /// Spawns the configured number of dispatcher tasks. Each dispatcher polls Redis
    /// for jobs and spawns tasks to process them, with the semaphore controlling
    /// maximum concurrency.
    ///
    /// This method is non-blocking and returns immediately after spawning dispatchers.
    pub async fn start(&self) -> Result<(), Error> {
        let mut dispatchers = self.dispatchers.write().await;

        if !dispatchers.is_empty() {
            tracing::warn!("Worker pool is already running");
            return Ok(());
        }

        let dispatcher_count = self.config.dispatcher_count();
        tracing::info!(
            "Starting worker pool with {} dispatchers (max {} concurrent jobs with queue prefetch batch: {})",
            dispatcher_count,
            self.config.max_concurrent_jobs,
            self.config.prefetch_batch_size()
        );

        // Spawn dispatchers with staggered delays to prevent thundering herd
        for id in 0..dispatcher_count {
            let handle = DispatcherHandle::spawn(id, self.config.clone(), self.context.clone());
            dispatchers.push(handle);

            // Add stagger delay between spawns (except after the last one)
            if id < dispatcher_count - 1 {
                let stagger_delay = self.config.dispatcher_spawn_stagger();
                tracing::debug!(
                    "Staggering dispatcher spawn: waiting {}ms before spawning next dispatcher",
                    stagger_delay.as_millis()
                );
                tokio::time::sleep(stagger_delay).await;
            }
        }

        tracing::info!(
            "Worker pool started successfully ({} dispatchers active)",
            dispatcher_count
        );
        Ok(())
    }

    /// Stop the worker pool gracefully
    ///
    /// Signals all dispatchers to stop and waits for them to complete. In-flight
    /// job-processing tasks will continue to completion naturally.
    ///
    /// This method blocks until all dispatchers have shut down.
    pub async fn stop(&self) -> Result<(), Error> {
        tracing::info!("Shutting down worker pool...");

        // Signal all dispatchers to stop
        self.context.shutdown_signal.notify_waiters();

        // Wait for all dispatchers to finish
        let mut dispatchers = self.dispatchers.write().await;
        let dispatcher_count = dispatchers.len();

        for dispatcher in dispatchers.drain(..) {
            tracing::debug!("Waiting for dispatcher {} to stop", dispatcher.id);
            if let Err(e) = dispatcher.handle.await {
                tracing::error!(
                    "Dispatcher {} failed to stop cleanly: {:?}",
                    dispatcher.id,
                    e
                );
            }
        }

        // Close the semaphore to prevent new tasks from starting
        self.context.semaphore.close();

        tracing::info!(
            "Worker pool shut down successfully ({} dispatchers stopped, in-flight tasks will complete)",
            dispatcher_count
        );
        Ok(())
    }

    /// Check if the worker pool is running
    pub async fn is_running(&self) -> bool {
        let dispatchers = self.dispatchers.read().await;
        !dispatchers.is_empty()
    }

    /// Get the number of active dispatchers
    pub async fn dispatcher_count(&self) -> usize {
        let dispatchers = self.dispatchers.read().await;
        dispatchers.len()
    }

    /// Get the number of available semaphore permits
    ///
    /// This indicates how many more jobs can be spawned before hitting the
    /// concurrency limit. A value of 0 means the system is at capacity.
    pub fn available_permits(&self) -> usize {
        self.context.available_permits()
    }

    /// Get the maximum number of concurrent jobs configured
    pub fn max_concurrent_jobs(&self) -> usize {
        self.config.max_concurrent_jobs
    }

    /// Get the current number of jobs being processed
    ///
    /// This is calculated as: max_concurrent_jobs - available_permits
    pub fn active_job_count(&self) -> usize {
        self.config.max_concurrent_jobs - self.context.available_permits()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        // Signal shutdown when pool is dropped
        self.context.shutdown_signal.notify_waiters();
        self.context.semaphore.close();
    }
}

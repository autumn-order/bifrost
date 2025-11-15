mod config;
mod context;
mod dispatcher;
mod supervisor;

pub mod handler;

pub use config::WorkerPoolConfig;

use std::sync::Arc;

use dioxus_logger::tracing;
use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult};
use tokio::sync::{RwLock, Semaphore};

use crate::server::{error::Error, worker::queue::WorkerJobQueue};

use self::{
    context::DispatcherContext, dispatcher::DispatcherHandle, supervisor::SupervisorHandle,
};

/// Worker pool for processing jobs from the WorkerJobQueue
///
/// Uses Tokio's work-stealing scheduler to efficiently distribute job processing
/// across available threads. Dispatchers poll Redis and spawn tasks directly,
/// with a semaphore controlling maximum concurrency.
pub struct WorkerPool {
    config: WorkerPoolConfig,
    context: DispatcherContext,
    dispatchers: Arc<RwLock<Vec<DispatcherHandle>>>,
    supervisor_handle: Arc<RwLock<Option<SupervisorHandle>>>,
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
            supervisor_handle: Arc::new(RwLock::new(None)),
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

        // Validate PostgreSQL connection pool capacity
        self.validate_db_pool_capacity().await;

        let dispatcher_count = self.config.dispatcher_count();
        tracing::info!(
            "Starting worker pool with {} dispatchers (max {} concurrent jobs with queue prefetch batch: {})",
            dispatcher_count,
            self.config.max_concurrent_jobs,
            self.config.prefetch_batch_size()
        );

        // Start the job queue cleanup task
        self.context.queue.start_cleanup().await;

        // Spawn all dispatchers immediately (each applies its own initial jitter to prevent thundering herd)
        for id in 0..dispatcher_count {
            let handle = DispatcherHandle::spawn(id, self.config.clone(), self.context.clone());
            dispatchers.push(handle);
        }

        tracing::info!(
            "Worker pool started successfully ({} dispatchers active)",
            dispatcher_count
        );

        // Release the dispatchers lock before starting supervisor
        drop(dispatchers);

        // Start the supervisor to monitor dispatcher health
        self.start_supervisor().await;

        Ok(())
    }

    /// Start the supervisor task that monitors dispatcher health
    ///
    /// The supervisor periodically checks if any dispatchers have died unexpectedly
    /// and respawns them to maintain the configured dispatcher count.
    async fn start_supervisor(&self) {
        let mut supervisor_handle = self.supervisor_handle.write().await;

        if supervisor_handle.is_some() {
            tracing::warn!("Supervisor is already running");
            return;
        }

        let handle = SupervisorHandle::spawn(
            self.config.clone(),
            self.context.clone(),
            Arc::clone(&self.dispatchers),
        );

        *supervisor_handle = Some(handle);
    }

    /// Stop the worker pool gracefully
    ///
    /// Signals all dispatchers to stop and waits for them to complete. In-flight
    /// job-processing tasks will continue to completion naturally.
    ///
    /// This method blocks until all dispatchers have shut down.
    pub async fn stop(&self) -> Result<(), Error> {
        tracing::info!("Shutting down worker pool...");

        // Set shutdown flag first so supervisor knows this is intentional
        self.context.set_shutting_down();

        // Signal shutdown so all tasks can see it and break out of their loops cleanly
        self.context.shutdown_signal.notify_waiters();

        // Wait for supervisor to stop
        let mut supervisor_handle = self.supervisor_handle.write().await;
        if let Some(handle) = supervisor_handle.take() {
            if let Err(e) = handle.shutdown().await {
                tracing::error!("Supervisor task failed: {:?}", e);
            }
        }

        // Give dispatchers and cleanup task a brief moment to observe the shutdown signal
        // This prevents them from interpreting semaphore closure as an error
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Now close the semaphore to prevent any new job spawns
        // Dispatchers already handle semaphore closure gracefully by returning jobs to buffer
        self.context.semaphore.close();

        // Stop the job queue cleanup task
        self.context.queue.stop_cleanup().await;

        // Wait for all dispatchers to finish
        let mut dispatchers = self.dispatchers.write().await;
        let dispatcher_count = dispatchers.len();
        let mut panicked_count = 0;
        let mut error_count = 0;

        for dispatcher in dispatchers.drain(..) {
            tracing::debug!("Waiting for dispatcher {} to stop", dispatcher.id);
            match dispatcher.handle.await {
                Ok(()) => {
                    // Dispatcher stopped cleanly
                }
                Err(e) if e.is_panic() => {
                    panicked_count += 1;
                    tracing::error!(
                        "Dispatcher {} panicked during execution: {:?}",
                        dispatcher.id,
                        e
                    );
                }
                Err(e) if e.is_cancelled() => {
                    tracing::warn!("Dispatcher {} was cancelled", dispatcher.id);
                }
                Err(e) => {
                    error_count += 1;
                    tracing::error!("Dispatcher {} failed with error: {:?}", dispatcher.id, e);
                }
            }
        }

        if panicked_count > 0 || error_count > 0 {
            tracing::warn!(
                "Worker pool shut down with issues ({} dispatchers stopped, {} panicked, {} errored)",
                dispatcher_count,
                panicked_count,
                error_count
            );
        } else {
            tracing::info!(
                "Worker pool shut down successfully ({} dispatchers stopped, in-flight tasks will complete)",
                dispatcher_count
            );
        }
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

    /// Validate that max_concurrent_jobs doesn't exceed 80% of PostgreSQL pool size
    async fn validate_db_pool_capacity(&self) {
        // Only validate for PostgreSQL connections
        match self.context.db.get_database_backend() {
            DatabaseBackend::Postgres => {
                if let Some(pool_size) = self.get_postgres_pool_size().await {
                    let recommended_max = (pool_size as f64 * 0.8) as usize;
                    let max_concurrent = self.config.max_concurrent_jobs;

                    if max_concurrent > recommended_max {
                        tracing::warn!(
                            "max concurrent jobs ({}) exceeds 80% of PostgreSQL connection pool size ({}). \
                             Pool size: {}, recommended max concurrent jobs: {}. \
                             This may lead to connection exhaustion and job failures.",
                            max_concurrent,
                            recommended_max,
                            pool_size,
                            recommended_max
                        );
                    } else {
                        tracing::debug!(
                            "PostgreSQL pool validation: max_concurrent_jobs ({}) is within safe limits \
                             ({}% of pool size {})",
                            max_concurrent,
                            (max_concurrent as f64 / pool_size as f64 * 100.0) as usize,
                            pool_size
                        );
                    }
                } else {
                    tracing::warn!(
                        "Unable to determine PostgreSQL connection pool size. \
                         Ensure max_concurrent_jobs ({}) doesn't exceed ~80% of your pool capacity.",
                        self.config.max_concurrent_jobs
                    );
                }
            }
            DatabaseBackend::Sqlite => {
                tracing::debug!("SQLite connection detected, skipping pool size validation");
            }
            DatabaseBackend::MySql => {
                tracing::debug!("MySQL connection detected, pool size validation not implemented");
            }
            _ => {
                tracing::debug!("Unknown database backend, skipping pool size validation");
            }
        }
    }

    /// Get PostgreSQL connection pool size
    async fn get_postgres_pool_size(&self) -> Option<usize> {
        #[derive(Debug, FromQueryResult)]
        struct MaxConnections {
            max_connections: String,
        }

        // Query PostgreSQL for max_connections setting
        let result = MaxConnections::find_by_statement(sea_orm::Statement::from_string(
            DatabaseBackend::Postgres,
            "SHOW max_connections".to_string(),
        ))
        .one(self.context.db.as_ref())
        .await;

        match result {
            Ok(Some(row)) => match row.max_connections.parse::<usize>() {
                Ok(max_conn) => Some(max_conn),
                Err(e) => {
                    tracing::debug!("Failed to parse max_connections value: {}", e);
                    None
                }
            },
            Ok(None) => {
                tracing::debug!("No result returned from SHOW max_connections query");
                None
            }
            Err(e) => {
                tracing::debug!("Failed to query PostgreSQL max_connections: {}", e);
                None
            }
        }
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        // Signal shutdown when pool is dropped
        self.context.shutdown_signal.notify_waiters();
        self.context.semaphore.close();
    }
}

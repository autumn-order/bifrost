use std::sync::Arc;

use dioxus_logger::tracing;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::{config::WorkerPoolConfig, context::DispatcherContext, dispatcher::DispatcherHandle};

/// Handle for the supervisor task that monitors dispatcher health
pub(super) struct SupervisorHandle {
    handle: JoinHandle<()>,
}

impl SupervisorHandle {
    /// Spawn a new supervisor task
    ///
    /// The supervisor periodically checks if any dispatchers have died unexpectedly
    /// and respawns them to maintain the configured dispatcher count.
    ///
    /// # Arguments
    /// - `config`: Worker pool configuration (contains check interval and dispatcher count)
    /// - `context`: Shared context with shutdown signal
    /// - `dispatchers`: Shared vec of dispatcher handles to monitor
    pub fn spawn(
        config: WorkerPoolConfig,
        context: DispatcherContext,
        dispatchers: Arc<RwLock<Vec<DispatcherHandle>>>,
    ) -> Self {
        let shutdown_signal = Arc::clone(&context.shutdown_signal);

        let handle = tokio::spawn(async move {
            tracing::info!("Worker supervisor started");
            Self::run_supervisor(config, context, dispatchers, shutdown_signal).await;
            tracing::info!("Worker supervisor stopped");
        });

        Self { handle }
    }

    /// Wait for the supervisor task to complete
    pub async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
        tracing::debug!("Waiting for worker supervisor to stop");
        self.handle.await
    }

    /// Main supervisor loop that monitors and respawns dead dispatchers
    async fn run_supervisor(
        config: WorkerPoolConfig,
        context: DispatcherContext,
        dispatchers: Arc<RwLock<Vec<DispatcherHandle>>>,
        shutdown_signal: Arc<tokio::sync::Notify>,
    ) {
        let check_interval = config.supervisor_check_interval();
        let expected_count = config.dispatcher_count();

        loop {
            tokio::select! {
                biased;

                _ = shutdown_signal.notified() => {
                    tracing::debug!("Supervisor received shutdown signal");
                    break;
                }

                _ = tokio::time::sleep(check_interval) => {
                    Self::check_and_respawn_dispatchers(
                        &config,
                        &context,
                        &dispatchers,
                        expected_count
                    ).await;
                }
            }
        }
    }

    /// Check for dead dispatchers and respawn them
    async fn check_and_respawn_dispatchers(
        config: &WorkerPoolConfig,
        context: &DispatcherContext,
        dispatchers: &Arc<RwLock<Vec<DispatcherHandle>>>,
        expected_count: usize,
    ) {
        let mut dispatchers = dispatchers.write().await;
        let mut respawned = 0;

        // Check for dead dispatchers (tasks that have finished)
        for i in (0..dispatchers.len()).rev() {
            if dispatchers[i].is_finished() {
                let dead_dispatcher = dispatchers.remove(i);
                let dispatcher_id = dead_dispatcher.id;

                // During shutdown, dispatchers exit normally - don't respawn or log errors
                if context.is_shutting_down() {
                    tracing::debug!(
                        "Dispatcher {} stopped during shutdown (expected)",
                        dispatcher_id
                    );
                    continue;
                }

                // Log failure reason by awaiting the finished handle
                Self::log_dispatcher_failure(dead_dispatcher).await;

                // Recover any jobs left in the dispatcher's buffer
                // This is critical with panic = "abort" since Drop handlers don't run
                Self::recover_dispatcher_buffer(context, dispatcher_id).await;

                // Respawn with same ID
                tracing::info!("Supervisor respawning dispatcher {}", dispatcher_id);
                let new_dispatcher =
                    DispatcherHandle::spawn(dispatcher_id, config.clone(), context.clone());
                dispatchers.insert(i, new_dispatcher);
                respawned += 1;
            }
        }

        if respawned > 0 {
            tracing::warn!(
                "Supervisor respawned {} dispatcher(s) ({}/{} now active)",
                respawned,
                dispatchers.len(),
                expected_count
            );
        }

        // Check if we're below expected count (shouldn't happen but defensive)
        // Skip this check during shutdown
        let current_count = dispatchers.len();
        if !context.is_shutting_down() && current_count < expected_count {
            tracing::error!(
                "Dispatcher count below expected ({}/{}), spawning additional dispatchers",
                current_count,
                expected_count
            );

            for id in current_count..expected_count {
                let new_dispatcher = DispatcherHandle::spawn(id, config.clone(), context.clone());
                dispatchers.push(new_dispatcher);
            }
        }
    }

    /// Log the reason a dispatcher failed by awaiting its handle
    async fn log_dispatcher_failure(dispatcher: DispatcherHandle) {
        let dispatcher_id = dispatcher.id;

        match dispatcher.handle.await {
            Ok(()) => {
                tracing::error!(
                    "Dispatcher {} unexpectedly stopped (normal exit during supervision)",
                    dispatcher_id
                );
            }
            Err(e) if e.is_panic() => {
                tracing::error!("Dispatcher {} panicked: {:?}", dispatcher_id, e);
            }
            Err(e) if e.is_cancelled() => {
                tracing::warn!("Dispatcher {} was cancelled", dispatcher_id);
            }
            Err(e) => {
                tracing::error!("Dispatcher {} failed with error: {:?}", dispatcher_id, e);
            }
        }
    }

    /// Recover jobs from a dead dispatcher's buffer
    ///
    /// When a dispatcher dies (especially via panic with abort), its buffer
    /// remains in the persistent storage. We drain it and return jobs to the queue.
    async fn recover_dispatcher_buffer(context: &DispatcherContext, dispatcher_id: usize) {
        match context.recover_buffer(dispatcher_id).await {
            Ok(0) => {
                tracing::debug!(
                    "Dispatcher {} buffer was empty (no jobs to recover)",
                    dispatcher_id
                );
            }
            Ok(recovered) => {
                tracing::info!(
                    "Recovered {} jobs from dispatcher {} buffer and returned to queue",
                    recovered,
                    dispatcher_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to recover buffer from dispatcher {}: {}",
                    dispatcher_id,
                    e
                );
            }
        }
    }
}

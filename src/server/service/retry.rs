//! Retry logic with exponential backoff for service operations.
//!
//! This module provides the `RetryContext` for executing operations with automatic retry
//! logic and exponential backoff. It supports caching between retry attempts to prevent
//! redundant fetches from ESI or database, and integrates with the error system to determine
//! which errors are retryable.

use std::time::Duration;

use dioxus_logger::tracing;

use crate::server::error::{retry::ErrorRetryStrategy, Error};

/// Context for executing operations with automatic retry logic and caching.
///
/// Provides exponential backoff retry behavior with configurable max attempts and initial
/// backoff duration. The generic cache type `T` persists data between retry attempts to
/// avoid redundant fetches from ESI or database queries.
///
/// # Type Parameters
///
/// - `T` - Cache type that must implement `Clone + Default`. Typically `OrchestrationCache`
///   for EVE data operations or `()` for operations without caching needs.
///
/// # Retry Behavior
///
/// - **Max attempts**: 3 (default)
/// - **Backoff strategy**: Exponential starting at 1 second (1s, 2s, 4s, ...)
/// - **Retry conditions**: Only errors with `ErrorRetryStrategy::Retry` are retried
/// - **Permanent failures**: Errors with `ErrorRetryStrategy::Fail` return immediately
///
/// # Example
///
/// ```ignore
/// let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();
/// let db = db.clone();
/// let esi_client = esi_client.clone();
///
/// ctx.execute_with_retry("info update for alliance ID 123", |cache| {
///     let db = db.clone();
///     let esi_client = esi_client.clone();
///
///     Box::pin(async move {
///         let alliance_orch = AllianceOrchestrator::new(&db, &esi_client);
///
///         // Fetch alliance - uses cache if already fetched in previous attempt
///         let fetched_alliance = alliance_orch.fetch_alliance(123, cache).await?;
///
///         // Persist within transaction
///         let txn = TrackedTransaction::begin(&db).await?;
///         let model = alliance_orch.persist(&txn, 123, fetched_alliance, cache).await?;
///         txn.commit().await?;
///
///         Ok(model)
///     })
/// }).await?;
/// ```
pub struct RetryContext<T> {
    /// Cache to be used between retries to prevent unnecessary additional fetches
    cache: T,
    /// Maximum number of attempts before giving up
    max_attempts: u32,
    /// Initial backoff duration in seconds (doubles with each retry)
    initial_backoff_secs: u64,
}

impl<T> RetryContext<T>
where
    T: Clone + Default,
{
    const DEFAULT_MAX_ATTEMPTS: u32 = 3;
    const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 1;

    /// Creates a new retry context with default configuration.
    ///
    /// Initializes a retry context with 3 max attempts and 1 second initial backoff.
    /// The cache is initialized using its `Default` implementation.
    ///
    /// # Returns
    /// - `RetryContext<T>` - New retry context with default settings
    pub fn new() -> Self {
        Self {
            cache: T::default(),
            max_attempts: Self::DEFAULT_MAX_ATTEMPTS,
            initial_backoff_secs: Self::DEFAULT_INITIAL_BACKOFF_SECS,
        }
    }

    /// Executes an operation with automatic retry logic and exponential backoff.
    ///
    /// Runs the provided async operation up to `max_attempts` times, retrying on transient
    /// failures with exponential backoff (1s, 2s, 4s, ...). The cache persists between
    /// retry attempts, allowing operations to skip redundant fetches.
    ///
    /// The operation should check the cache for existing data and populate it with fetched
    /// data to optimize retry attempts. Errors are evaluated using `to_retry_strategy()` to
    /// determine if they are retryable or permanent failures.
    ///
    /// # Arguments
    /// - `description` - Human-readable description for logging (e.g., "alliance info update")
    /// - `operation` - Async function that receives mutable cache reference and returns `Result<R, Error>`
    ///
    /// # Returns
    /// - `Ok(R)` - Operation succeeded
    /// - `Err(Error)` - Operation failed permanently or exhausted all retry attempts
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();
    /// let db = db.clone();
    /// let esi_client = esi_client.clone();
    ///
    /// ctx.execute_with_retry("info update for corporation ID 456", |cache| {
    ///     let db = db.clone();
    ///     let esi_client = esi_client.clone();
    ///
    ///     Box::pin(async move {
    ///         let corporation_orch = CorporationOrchestrator::new(&db, &esi_client);
    ///
    ///         // Fetch corporation (checks cache first, uses cached data on retry)
    ///         let fetched_corporation = corporation_orch
    ///             .fetch_corporation(456, cache)
    ///             .await?;
    ///
    ///         // Persist within transaction (idempotent due to cache tracking)
    ///         let txn = TrackedTransaction::begin(&db).await?;
    ///         let model = corporation_orch
    ///             .persist(&txn, 456, fetched_corporation, cache)
    ///             .await?;
    ///         txn.commit().await?;
    ///
    ///         Ok(model)
    ///     })
    /// }).await?;
    /// ```
    pub async fn execute_with_retry<R, F>(
        &mut self,
        description: &str,
        operation: F,
    ) -> Result<R, Error>
    where
        F: for<'a> Fn(
            &'a mut T,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<R, Error>> + Send + 'a>,
        >,
    {
        let mut attempt_count = 0;

        loop {
            tracing::debug!(
                "Processing {} (attempt {}/{})",
                description,
                attempt_count + 1,
                self.max_attempts
            );

            // Execute the operation, passing db, esi_client, and cached data if available
            let result = operation(&mut self.cache).await;

            match result {
                // Return result R
                Ok(result) => {
                    tracing::debug!("Successfully processed {}", description);
                    return Ok(result);
                }
                Err(e) => match e.to_retry_strategy() {
                    ErrorRetryStrategy::Fail => {
                        tracing::error!("Permanent error for {}: {:?}", description, e);
                        return Err(e);
                    }
                    ErrorRetryStrategy::Retry => {
                        attempt_count += 1;
                        if attempt_count >= self.max_attempts {
                            tracing::error!(
                                "Max attempts ({}) exceeded for {}: {:?}",
                                self.max_attempts,
                                description,
                                e
                            );
                            return Err(e);
                        }

                        let backoff_secs = self.initial_backoff_secs * 2_u64.pow(attempt_count - 1);
                        let backoff = Duration::from_secs(backoff_secs);

                        tracing::warn!(
                            "Retrying {} (attempt {}/{}) after {:?}: {:?}",
                            description,
                            attempt_count,
                            self.max_attempts,
                            backoff,
                            e
                        );

                        tokio::time::sleep(backoff).await;
                    }
                },
            }
        }
    }
}

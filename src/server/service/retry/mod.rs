pub mod cache;

use std::time::Duration;

use dioxus_logger::tracing;

use crate::server::error::{retry::ErrorRetryStrategy, Error};

/// Context for service methods providing retry & caching logic
pub struct RetryContext<T> {
    /// cache to be used between retries to prevent unnecessary additional fetches
    cache: T,
    /// Max attempts before failure
    max_attempts: u32,
    /// Initial backoff between attempts
    initial_backoff_secs: u64,
}

impl<T> RetryContext<T>
where
    T: Clone + Default,
{
    const DEFAULT_MAX_ATTEMPTS: u32 = 3;
    const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 1;

    pub fn new() -> Self {
        Self {
            cache: T::default(),
            max_attempts: Self::DEFAULT_MAX_ATTEMPTS,
            initial_backoff_secs: Self::DEFAULT_INITIAL_BACKOFF_SECS,
        }
    }

    /// Execute a method with automatic retry logic
    ///
    /// The operation function receives:
    /// - `retry_cache`: Optional cached data from previous attempt(s)
    ///
    /// The operation should:
    /// - Use cached_data if available to skip additional fetches
    /// - Fetch from ESI if cached_data is None
    /// - Store to database
    ///
    /// # Arguments
    /// - `description`: Description of the operation for logging (e.g., "alliance info update")
    /// - `operation`: Async function that performs fetch and store
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

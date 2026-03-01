//! ESI endpoint group circuit breaker and rate limit tracking.
//!
//! This module implements the `EndpointGroup` struct that manages circuit breaker state
//! and rate limit tracking for groups of related ESI endpoints. The actual state machine
//! logic is defined in the [`status`](super::status) module via the `EndpointStatus` enum.
//!
//! ## Architecture
//!
//! The `EndpointGroup` serves as a coordinator that:
//! - Wraps an `EndpointStatus` enum (defined in [`status`](super::status)) behind a RwLock
//! - Provides thread-safe methods for checking and updating endpoint health
//! - Uses an atomic flag to coordinate recovery attempts across concurrent requests
//! - Delegates state machine logic to `EndpointStatus` methods
//!
//! ## State Machine
//!
//! The endpoint group can be in one of five states (see [`EndpointStatus`](super::status::EndpointStatus)):
//!
//! - `Healthy`: Normal operation, no recent errors, not rate limited
//! - `Impaired`: Errors detected, tracking requests in sliding window
//! - `Recovering`: Attempting to recover from Offline state after cooldown
//! - `Offline`: Circuit breaker tripped, ALL requests blocked until cooldown expires
//! - `RateLimited`: Hit ESI rate limit (429), PUBLIC requests blocked until retry_after expires
//!
//! For detailed state transition rules and examples, see the [`status`](super::status) module.
//!
//! ## Circuit Breaker (5xx Error Tracking)
//!
//! The circuit breaker uses a sliding window to track the last N request outcomes
//! (success or failure). This provides volume-independent failure detection that
//! works equally well for low-volume and high-volume endpoints. When the error rate
//! within the window exceeds the threshold, the circuit breaker trips to Offline state.
//!
//! **Circuit breaker affects ALL requests** - both public and authenticated.
//!
//! ## Rate Limit Tracking (429 Response Handling)
//!
//! ESI has two types of rate limits:
//! - **Public endpoints**: Share a rate limit pool per endpoint group (e.g., all character endpoints)
//! - **Authenticated endpoints**: Have independent rate limits per access token
//!
//! When a **public request** (no access token) receives a 429 response with `retry_after`:
//! - If endpoint is `Offline`: Rate limit info is preserved in the `Offline` state
//! - Otherwise: Endpoint transitions to `RateLimited` state
//!
//! When an **authenticated request** (with access token) receives a 429 response:
//! - Endpoint state is NOT affected (authenticated requests have independent rate limits)
//! - The 429 error is returned to the caller but doesn't block other requests
//!
//! **Rate limit tracking and blocking only applies to public requests**.
//! Authenticated requests bypass the `RateLimited` check entirely.
//!
//! ### State Priority
//!
//! `Offline` takes precedence over `RateLimited`:
//! - `Offline`: Blocks ALL requests (public + authenticated)
//! - `RateLimited`: Blocks only public requests (no access token)
//!
//! When recovering from `Offline` with a stored rate limit:
//! - Authenticated requests: Proceed to `Recovering` state
//! - Public requests: Transition to `RateLimited` if rate limit hasn't expired
//!
//! ## Concurrency
//!
//! The implementation uses:
//! - A **RwLock** wrapping `EndpointStatus` for status management
//!   - Multiple concurrent readers during healthy operation (no lock contention)
//!   - Single writer for state transitions
//! - An **AtomicBool** (`recovering` flag) to coordinate recovery attempts
//!   - Uses compare-and-swap (CAS) to ensure only one thread attempts recovery
//!   - Prevents thundering herd when circuit breaker reopens
//!
//! This design provides thread-safe operation with minimal lock contention.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Utc;
use dioxus_logger::tracing;
use eve_esi::EsiError;
use tokio::sync::RwLock;

use crate::server::{error::AppError, service::eve::esi::status::EndpointStatus};

/// Result of checking endpoint status and beginning recovery if needed.
///
/// This struct captures the state of the endpoint at the time of the check,
/// allowing the caller to make decisions about handling success responses
/// without requiring additional lock acquisitions.
#[derive(Debug, Clone, Copy)]
pub(super) struct CheckResult {
    /// Whether the endpoint is attempting recovery from Offline state
    pub attempting_recovery: bool,
    /// Whether the endpoint was in Impaired state at the time of check
    pub was_impaired: bool,
}

/// Circuit breaker for an ESI endpoint group.
///
/// Manages the health status of a group of related ESI endpoints (e.g., all character endpoints).
/// Wraps an [`EndpointStatus`](super::status::EndpointStatus) state machine to track errors and
/// automatically disable endpoints when error thresholds are exceeded, then re-enable them after
/// a cooldown period.
///
/// # Thread Safety
///
/// The `recovering` flag ensures only one concurrent request attempts recovery at a time,
/// preventing thundering herd problems when the circuit breaker reopens. The `status` field
/// is protected by a RwLock, allowing multiple concurrent readers during healthy operation.
pub struct EndpointGroup {
    /// Name of the endpoint group for logging context
    name: &'static str,
    /// Current health status of the endpoint group
    status: RwLock<EndpointStatus>,
    /// Atomic flag indicating whether a recovery attempt is in progress
    recovering: AtomicBool,
}

impl EndpointGroup {
    /// Creates a new endpoint group with the specified name.
    ///
    /// # Arguments
    /// - `name` - Name of the endpoint group for logging context
    ///
    /// # Returns
    /// New `EndpointGroup` with healthy initial state
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            status: RwLock::new(EndpointStatus::Healthy),
            recovering: AtomicBool::new(false),
        }
    }

    /// Check endpoint status and atomically begin recovery if eligible.
    ///
    /// This method performs a two-phase check:
    /// 1. Fast path: Acquire read lock to check current status
    /// 2. If recovery is eligible: Use atomic CAS to ensure only one thread begins recovery
    ///
    /// The returned `CheckResult` captures the state at check time, allowing the caller
    /// to optimize success handling without additional lock acquisitions.
    ///
    /// # Arguments
    /// - `has_access_token` - Whether the request has an access token (authenticated)
    ///
    /// # Returns
    /// - `Ok(CheckResult)` - Status checked successfully, contains state information
    /// - `Err(AppError::EsiEndpointOffline)` - Endpoint is offline and cooldown hasn't expired
    /// - `Err(AppError::EsiRateLimited)` - Endpoint is rate limited (public requests only)
    ///
    /// # Behavior
    /// - If `Healthy` or `Impaired`: Returns immediately with current state
    /// - If `Offline` and cooldown not expired: Returns error
    /// - If `Offline` and cooldown expired: Atomically transitions to `Recovering` or `RateLimited`
    /// - If `RateLimited` and has access token: Bypasses check (authenticated requests exempt)
    /// - If `RateLimited` and no access token and not expired: Returns error
    /// - If `RateLimited` and expired: Transitions to `Healthy`
    pub(super) async fn check_and_begin_recovery(
        &self,
        has_access_token: bool,
    ) -> Result<CheckResult, AppError> {
        // Fast path: read lock to check status
        let (attempting_recovery, is_impaired) = {
            let status = self.status.read().await;
            let attempting_recovery = status.check_recovery_eligibility(has_access_token)?;
            let is_impaired = matches!(*status, EndpointStatus::Impaired { .. });
            (attempting_recovery, is_impaired)
        };

        if attempting_recovery {
            // Use CAS to ensure only one thread attempts recovery
            if self
                .recovering
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // We won the race - transition to Recovering
                let mut status = self.status.write().await;

                // Double-check state hasn't changed - handle both Offline and RateLimited
                if matches!(
                    *status,
                    EndpointStatus::Offline { .. } | EndpointStatus::RateLimited { .. }
                ) {
                    tracing::info!(
                        group = %self.name,
                        "ESI endpoint group beginning recovery attempt after cooldown/rate limit period"
                    );
                    status.begin_recovery_attempt(has_access_token);
                } else {
                    // Someone else already transitioned, release flag
                    self.recovering.store(false, Ordering::Release);
                }
            }
            // If CAS failed, another thread is handling recovery
        }

        Ok(CheckResult {
            attempting_recovery,
            was_impaired: is_impaired,
        })
    }

    /// Records a rate limit for this endpoint group.
    ///
    /// This method is called when a 429 (Too Many Requests) response is received from a
    /// **public request** (no access token). Authenticated requests have independent rate
    /// limits per token and should not call this method.
    ///
    /// The rate limit is stored as an absolute timestamp based on the retry_after duration.
    /// If the endpoint is already `Offline`, the rate limit info is preserved for proper
    /// state transition after cooldown expires.
    ///
    /// # Arguments
    /// - `retry_after` - Duration until the rate limit expires
    pub(super) async fn handle_rate_limit(&self, retry_after: Duration) {
        let until = Utc::now() + chrono::Duration::from_std(retry_after).unwrap();

        let mut status = self.status.write().await;

        // If already offline, preserve the offline state but store rate limit info
        match &*status {
            EndpointStatus::Offline { last_error, .. } => {
                *status = EndpointStatus::Offline {
                    last_error: *last_error,
                    rate_limit_until: Some(until),
                };
                tracing::warn!(
                    group = self.name,
                    retry_after_secs = retry_after.as_secs(),
                    until = %until,
                    "Endpoint group rate limited while offline - rate limit info preserved"
                );
            }
            _ => {
                *status = EndpointStatus::RateLimited { until };
                tracing::warn!(
                    group = self.name,
                    retry_after_secs = retry_after.as_secs(),
                    until = %until,
                    "Endpoint group rate limited"
                );
            }
        }
    }

    /// Handle a 5xx error response from ESI.
    ///
    /// Updates the endpoint status based on the current state and error history.
    /// May transition the endpoint to a degraded state (Impaired or Offline) if
    /// error thresholds are exceeded.
    ///
    /// Delegates to [`EndpointStatus::handle_5xx_error`](super::status::EndpointStatus::handle_5xx_error)
    /// for state machine logic.
    ///
    /// # State transitions
    /// - `Healthy` → `Impaired`: First error recorded
    /// - `Impaired` → `Impaired` or `Offline`: Error accumulated, may trip circuit breaker
    /// - `Recovering` → `Offline`: Strict fail-fast behavior during recovery
    /// - `Offline` → `Offline`: No change (already offline)
    ///
    /// This method automatically clears the recovery flag if the endpoint transitions
    /// to Offline during recovery, allowing a new recovery attempt after the next cooldown.
    pub(super) async fn handle_5xx_error(&self) {
        let mut status = self.status.write().await;
        let was_recovering = matches!(*status, EndpointStatus::Recovering { .. });
        let old_state = format!("{:?}", *status);

        status.handle_5xx_error(self.name);

        // Log state transition if it occurred
        let new_state = format!("{:?}", *status);
        if old_state != new_state {
            tracing::debug!(
                group = %self.name,
                old_state = %old_state,
                new_state = %new_state,
                "ESI endpoint group state transition after 5xx error"
            );
        }

        // If we just went Offline during recovery, clear the flag for next attempt
        if matches!(*status, EndpointStatus::Offline { .. }) && was_recovering {
            self.recovering.store(false, Ordering::Release);
        }
    }

    /// Record a successful request outcome and potentially reset to healthy state.
    ///
    /// This method is called after a successful request to update the sliding window
    /// and potentially reset the endpoint from `Impaired` or `Recovering` back to `Healthy`.
    ///
    /// Delegates to [`EndpointStatus::handle_success`](super::status::EndpointStatus::handle_success)
    /// for state machine logic.
    ///
    /// # Arguments
    /// - `check_result` - The result from `check_and_begin_recovery()`, indicating state at check time
    ///
    /// # Behavior
    /// - If `check_result` indicates `Healthy`: No-op, no locks acquired (fast path)
    /// - If `Recovering`: Immediately resets to `Healthy` on success
    /// - If `Impaired`: Records success in sliding window, resets to `Healthy` if error rate drops below threshold
    pub(super) async fn handle_success(&self, check_result: CheckResult) {
        if check_result.attempting_recovery || check_result.was_impaired {
            let mut status = self.status.write().await;
            let was_recovering = matches!(*status, EndpointStatus::Recovering { .. });

            status.handle_success(self.name);

            // If we successfully recovered to Healthy from Recovering, clear the recovery flag
            if was_recovering && matches!(*status, EndpointStatus::Healthy) {
                self.recovering.store(false, Ordering::Release);
            }
        }
    }

    /// Handle EsiErrors with status 5xx & 429 returned from an EsiRequest.
    ///
    /// This method centralizes error handling logic for ESI requests, dispatching
    /// to the appropriate handler based on the error status code.
    ///
    /// # Arguments
    /// - `error` - An EsiError containing status code & rate limit info if applicable
    /// - `has_access_token` - Bool representing whether this was an authenticated ESI request
    ///
    /// # Behavior
    /// - **429 (Rate Limited)**: Only tracks rate limits for **public requests**.
    ///   Authenticated requests have independent rate limits per token and are not tracked.
    /// - **5xx errors**: Updates circuit breaker state via [`handle_5xx_error`](Self::handle_5xx_error),
    ///   may transition to Impaired or Offline.
    pub(super) async fn handle_esi_error(&self, error: &EsiError, has_access_token: bool) {
        if error.status == 429 {
            tracing::debug!("ESI request returned 429 rate limit error");
            if let Some(retry_after) = error.retry_after {
                if !has_access_token {
                    self.handle_rate_limit(retry_after).await;
                }
            }
        } else if matches!(error.status, 500..=599) {
            tracing::debug!(
                status = %error.status,
                "ESI request returned 5xx error, updating circuit breaker state"
            );
            self.handle_5xx_error().await;
        }
    }
}

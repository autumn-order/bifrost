//! ESI endpoint group circuit breaker and rate limit tracking.
//!
//! This module implements a unified state machine for ESI endpoint groups that handles
//! both circuit breaking (5xx error tracking) and rate limiting (429 responses).
//!
//! ## State Machine
//!
//! The endpoint group can be in one of five states:
//!
//! - `Healthy`: Normal operation, no recent errors, not rate limited
//! - `Impaired`: Errors detected, tracking requests in sliding window
//! - `Recovering`: Attempting to recover from Offline state after cooldown
//! - `Offline`: Circuit breaker tripped, ALL requests blocked until cooldown expires
//! - `RateLimited`: Hit ESI rate limit (429), PUBLIC requests blocked until retry_after expires
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
//! The implementation uses a single read-write lock for status management and an atomic
//! flag to coordinate recovery attempts across concurrent requests, ensuring thread-safe
//! operation with minimal lock contention.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
use eve_esi::EsiError;
use tokio::sync::RwLock;

use super::{
    ENDPOINT_GROUP_ERROR_RATE_THRESHOLD, ENDPOINT_GROUP_RETRY_COOLDOWN,
    ENDPOINT_GROUP_SLIDING_WINDOW_SIZE,
};
use crate::server::error::AppError;

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
/// Uses a state machine to track errors and automatically disable endpoints when error thresholds
/// are exceeded, then re-enable them after a cooldown period.
///
/// The `recovering` flag ensures only one concurrent request attempts recovery at a time,
/// preventing thundering herd problems when the circuit breaker reopens.
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
}

/// Health status of an ESI endpoint group.
///
/// Represents the current state in the circuit breaker and rate limit state machine:
///
/// # State Transitions
///
/// ## From `Healthy`
/// - → `Impaired`: First 5xx error
/// - → `RateLimited`: 429 response (public request only)
///
/// ## From `Impaired`
/// - → `Healthy`: Error rate drops below threshold
/// - → `Offline`: Error rate exceeds threshold
/// - → `RateLimited`: 429 response (public request only)
///
/// ## From `Offline`
/// - → `Recovering`: Cooldown expires (authenticated request or no active rate limit)
/// - → `RateLimited`: Cooldown expires but rate limit still active (public request only)
/// - → stays `Offline`: 429 response preserves rate limit info
///
/// ## From `Recovering`
/// - → `Healthy`: Successful request
/// - → `Offline`: Any error (strict fail-fast)
/// - → `RateLimited`: 429 response (public request only)
///
/// ## From `RateLimited`
/// - → `Healthy`: Rate limit expires and request succeeds
/// - Bypassed entirely by authenticated requests (independent rate limits per token)
#[derive(Debug, Clone)]
pub(super) enum EndpointStatus {
    /// Normal operation, no recent errors
    Healthy,
    /// Errors detected, tracking request outcomes in sliding window
    Impaired {
        /// Timestamp of first error (kept for observability/debugging)
        #[allow(dead_code)]
        first_error: DateTime<Utc>,
        /// Sliding window of recent request outcomes (true = success, false = error)
        /// Most recent requests are at the back of the deque
        recent_requests: VecDeque<bool>,
    },
    /// Attempting to recover from Offline state after cooldown
    Recovering,
    /// Circuit breaker tripped, requests blocked until cooldown expires
    Offline {
        /// Timestamp when endpoint went offline (used for cooldown calculation)
        last_error: DateTime<Utc>,
        /// Optional rate limit expiration time if we were rate limited when offline
        /// This is preserved so we can transition to RateLimited instead of Recovering
        /// when the circuit breaker cooldown expires but rate limit is still active.
        rate_limit_until: Option<DateTime<Utc>>,
    },
    /// Rate limited, requests blocked until retry_after expires
    RateLimited {
        /// Timestamp when rate limit expires
        until: DateTime<Utc>,
    },
}

impl EndpointGroup {
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
    /// This method is called when a 429 (Too Many Requests) response is received.
    /// The rate limit is stored as an absolute timestamp based on the retry_after duration.
    /// Transitions the endpoint to `RateLimited` state regardless of current circuit breaker state.
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
    /// # State transitions
    /// - `Healthy` → `Impaired`: First error recorded
    /// - `Impaired` → `Impaired` or `Offline`: Error accumulated, may trip circuit breaker
    /// - `Recovering` → `Recovering` or `Offline`: Stricter threshold applied (3 errors max)
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
    /// This method centralizes error handling logic for ESI requests, updating
    /// the endpoint group's circuit breaker state appropriately.
    ///
    /// # Arguments
    /// - `error` - An EsiError containing status code & rate limit info if applicable
    /// - `has_access_token` - Bool representing whether this was an authenticated ESI request
    ///
    /// # Behavior
    /// - **429 (Rate Limited)**: Only tracks rate limits for public requests.
    ///   Authenticated requests have independent rate limits per token.
    /// - **5xx errors**: Updates circuit breaker state, may transition to Impaired or Offline.
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

impl EndpointStatus {
    /// Check if the endpoint is offline and eligible for recovery.
    ///
    /// This method determines whether a recovery attempt should be made based on
    /// the endpoint's current status and cooldown timing.
    ///
    /// # Arguments
    /// - `has_access_token` - Whether the request has an access token (authenticated)
    ///
    /// # Returns
    /// - `Ok(true)` - Endpoint is eligible for recovery/transition
    /// - `Ok(false)` - Endpoint is healthy/impaired (no recovery needed)
    /// - `Err(AppError::EsiEndpointOffline)` - Endpoint is `Offline` but cooldown hasn't expired
    /// - `Err(AppError::EsiRateLimited)` - Endpoint is rate limited (public requests only)
    ///
    /// # Note
    /// This is a read-only check. Actual state transition to `Recovering` or `RateLimited`
    /// is performed by `begin_recovery_attempt()` after this check passes.
    pub(super) fn check_recovery_eligibility(
        &self,
        has_access_token: bool,
    ) -> Result<bool, AppError> {
        match self {
            EndpointStatus::RateLimited { until } => {
                // Authenticated requests bypass rate limit (they have independent rate limits per token)
                if has_access_token {
                    return Ok(false);
                }

                let now = Utc::now();
                if now < *until {
                    let remaining = (*until - now).to_std().unwrap_or(Duration::from_secs(0));
                    Err(AppError::EsiRateLimited {
                        retry_after: Some(remaining),
                    })
                } else {
                    // Rate limit has expired, allow request and signal transition to Healthy
                    Ok(true)
                }
            }
            EndpointStatus::Offline {
                last_error,
                rate_limit_until,
            } => {
                let now = Utc::now();
                let elapsed = now.signed_duration_since(*last_error);

                if elapsed.to_std().unwrap_or(Duration::ZERO) < ENDPOINT_GROUP_RETRY_COOLDOWN {
                    // Circuit breaker cooldown not expired yet
                    Err(AppError::EsiEndpointOffline)
                } else {
                    // Cooldown has passed
                    // If we have a rate limit and this is a public request, check if rate limit is still active
                    if !has_access_token {
                        if let Some(until) = rate_limit_until {
                            if now < *until {
                                // Rate limit still active for public requests, signal transition to RateLimited
                                return Ok(true);
                            }
                        }
                    }
                    // Either authenticated request, or rate limit expired, allow retry and signal recovery attempt
                    Ok(true)
                }
            }
            _ => Ok(false),
        }
    }

    /// Transition from Offline to Recovering state, or to RateLimited if rate limit is still active.
    ///
    /// This method is called after `check_recovery_eligibility()` returns `Ok(true)`
    /// to atomically transition the endpoint into recovery mode.
    ///
    /// # Arguments
    /// - `has_access_token` - Whether the request has an access token (authenticated)
    ///
    /// # Behavior
    /// - If current state is `Offline` with active rate limit (public request): Transitions to `RateLimited`
    /// - If current state is `Offline` without rate limit or authenticated: Transitions to `Recovering`
    /// - If current state is `RateLimited` and expired: Transitions to `Healthy`
    /// - Otherwise: No-op (another thread may have already transitioned)
    pub(super) fn begin_recovery_attempt(&mut self, has_access_token: bool) {
        match self {
            EndpointStatus::Offline {
                rate_limit_until: Some(until),
                ..
            } => {
                let now = Utc::now();
                // If rate limit is still active and this is a public request, transition to RateLimited
                if !has_access_token && now < *until {
                    *self = EndpointStatus::RateLimited { until: *until };
                } else {
                    // Rate limit expired or authenticated request, proceed to recovery
                    *self = EndpointStatus::Recovering;
                }
            }
            EndpointStatus::Offline {
                rate_limit_until: None,
                ..
            } => {
                // No rate limit, transition to Recovering
                *self = EndpointStatus::Recovering;
            }
            EndpointStatus::RateLimited { .. } => {
                // Rate limit has expired, transition back to Healthy
                *self = EndpointStatus::Healthy;
            }
            _ => {}
        }
    }

    /// Process a 5xx error and update the status accordingly.
    ///
    /// Implements the core circuit breaker logic for handling errors. The behavior
    /// varies based on current state:
    ///
    /// # State-specific behavior
    ///
    /// ## Healthy
    /// Transitions to `Impaired` with a sliding window containing one error.
    ///
    /// ## Impaired
    /// Adds the error to the sliding window and checks if error rate exceeds threshold:
    /// - If error rate > threshold: Transitions to `Offline`
    /// - Otherwise: Remains `Impaired` with updated window
    ///
    /// ## Recovering
    /// Applies strict fail-fast behavior - any error immediately transitions back to `Offline`.
    /// This prevents prolonged recovery attempts when the endpoint is still broken.
    ///
    /// ## Offline
    /// No-op, remains `Offline` until cooldown expires.
    pub(super) fn handle_5xx_error(&mut self, group_name: &str) {
        let now = Utc::now();

        match self {
            EndpointStatus::Healthy => {
                tracing::debug!(
                    group = %group_name,
                    "ESI endpoint group transitioned from Healthy to Impaired after first 5xx error"
                );
                let mut window = VecDeque::with_capacity(ENDPOINT_GROUP_SLIDING_WINDOW_SIZE);
                window.push_back(false); // false = error
                *self = EndpointStatus::Impaired {
                    first_error: now,
                    recent_requests: window,
                };
            }
            EndpointStatus::Impaired {
                first_error: _,
                recent_requests,
            } => {
                // Add error to sliding window
                recent_requests.push_back(false); // false = error

                // Maintain window size
                if recent_requests.len() > ENDPOINT_GROUP_SLIDING_WINDOW_SIZE {
                    recent_requests.pop_front();
                }

                // Calculate error rate
                let error_count = recent_requests.iter().filter(|&&success| !success).count();
                let total_requests = recent_requests.len();
                let error_rate = error_count as f64 / total_requests as f64;

                if error_rate >= ENDPOINT_GROUP_ERROR_RATE_THRESHOLD {
                    tracing::error!(
                        group = %group_name,
                        error_count = %error_count,
                        total_requests = %total_requests,
                        error_rate = %format!("{:.1}%", error_rate * 100.0),
                        threshold = %format!("{:.1}%", ENDPOINT_GROUP_ERROR_RATE_THRESHOLD * 100.0),
                        cooldown_seconds = %ENDPOINT_GROUP_RETRY_COOLDOWN.as_secs(),
                        "ESI endpoint group circuit breaker tripped - error rate exceeded threshold; endpoint now offline"
                    );
                    *self = EndpointStatus::Offline {
                        last_error: now,
                        rate_limit_until: None,
                    };
                } else {
                    tracing::debug!(
                        group = %group_name,
                        error_count = %error_count,
                        total_requests = %total_requests,
                        error_rate = %format!("{:.1}%", error_rate * 100.0),
                        "ESI endpoint group recorded error in sliding window"
                    );
                    // State remains Impaired with updated window
                }
            }
            EndpointStatus::Recovering { .. } => {
                // Strict fail-fast during recovery - any error immediately returns to offline
                tracing::error!(
                    group = %group_name,
                    "ESI endpoint group failed recovery - error during recovery attempt; returning to offline state"
                );
                *self = EndpointStatus::Offline {
                    last_error: now,
                    rate_limit_until: None,
                };
            }
            EndpointStatus::Offline { .. } => {}
            EndpointStatus::RateLimited { .. } => {
                // If we somehow get a 5xx error while rate limited, stay rate limited
                // The rate limit takes precedence
            }
        }
    }

    /// Record a successful request outcome.
    ///
    /// This method should be called after successful 2xx responses to update the
    /// sliding window and potentially reset to healthy state.
    ///
    /// # State-specific behavior
    ///
    /// ## Healthy
    /// No-op, already healthy.
    ///
    /// ## Impaired
    /// Adds success to sliding window. If error rate drops below threshold, transitions to `Healthy`.
    ///
    /// ## Recovering
    /// Immediately transitions to `Healthy` on first success.
    ///
    /// ## Offline
    /// No-op, must go through recovery first.
    pub(super) fn handle_success(&mut self, group_name: &str) {
        match self {
            EndpointStatus::Healthy => {
                // No-op, already healthy
            }
            EndpointStatus::Impaired {
                recent_requests, ..
            } => {
                // Add success to sliding window
                recent_requests.push_back(true); // true = success

                // Maintain window size
                if recent_requests.len() > ENDPOINT_GROUP_SLIDING_WINDOW_SIZE {
                    recent_requests.pop_front();
                }

                // Calculate error rate
                let error_count = recent_requests.iter().filter(|&&success| !success).count();
                let total_requests = recent_requests.len();
                let error_rate = error_count as f64 / total_requests as f64;

                if error_rate < ENDPOINT_GROUP_ERROR_RATE_THRESHOLD {
                    tracing::info!(
                        group = %group_name,
                        error_count = %error_count,
                        total_requests = %total_requests,
                        error_rate = %format!("{:.1}%", error_rate * 100.0),
                        "ESI endpoint group recovered to healthy state - error rate below threshold"
                    );
                    *self = EndpointStatus::Healthy;
                } else {
                    tracing::debug!(
                        group = %group_name,
                        error_count = %error_count,
                        total_requests = %total_requests,
                        error_rate = %format!("{:.1}%", error_rate * 100.0),
                        "ESI endpoint group recorded success in sliding window, but error rate still above threshold"
                    );
                }
            }
            EndpointStatus::Recovering { .. } => {
                // Immediate success during recovery = back to healthy
                tracing::info!(
                    group = %group_name,
                    "ESI endpoint group successfully recovered to healthy state"
                );
                *self = EndpointStatus::Healthy;
            }
            EndpointStatus::Offline { .. } => {}
            EndpointStatus::RateLimited { .. } => {
                // Rate limit has expired and request succeeded, transition to Healthy
                tracing::info!(
                    group = %group_name,
                    "ESI endpoint group rate limit expired, transitioning to Healthy"
                );
                *self = EndpointStatus::Healthy;
            }
        }
    }
}

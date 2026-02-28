//! ESI endpoint group circuit breaker implementation.
//!
//! This module implements a circuit breaker pattern for ESI endpoint groups,
//! tracking error rates and automatically disabling/re-enabling endpoints based
//! on their health status. The circuit breaker has four states:
//!
//! - `Healthy`: Normal operation, no recent errors
//! - `Impaired`: Errors detected, tracking requests in sliding window
//! - `Recovering`: Attempting to recover from Offline state after cooldown
//! - `Offline`: Circuit breaker tripped, requests blocked until cooldown expires
//!
//! The implementation uses a read-write lock for status management and an atomic
//! flag to coordinate recovery attempts across concurrent requests.
//!
//! ## Sliding Window Approach
//!
//! The circuit breaker uses a sliding window to track the last N request outcomes
//! (success or failure). This provides volume-independent failure detection that
//! works equally well for low-volume and high-volume endpoints. When the error rate
//! within the window exceeds the threshold, the circuit breaker trips.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
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
/// Represents the current state in the circuit breaker state machine:
///
/// State transitions:
/// - `Healthy` → `Impaired`: First 5xx error occurs
/// - `Impaired` → `Healthy`: Error rate drops below threshold (successful requests)
/// - `Impaired` → `Offline`: Error rate exceeds threshold in sliding window
/// - `Offline` → `Recovering`: Cooldown expires and request attempts recovery
/// - `Recovering` → `Healthy`: Successful request during recovery
/// - `Recovering` → `Offline`: Error during recovery (strict fail-fast behavior)
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
    /// # Returns
    /// - `Ok(CheckResult)` - Status checked successfully, contains state information
    /// - `Err(AppError::EsiEndpointOffline)` - Endpoint is offline and cooldown hasn't expired
    ///
    /// # Behavior
    /// - If `Healthy` or `Impaired`: Returns immediately with current state
    /// - If `Offline` and cooldown not expired: Returns error
    /// - If `Offline` and cooldown expired: Atomically transitions to `Recovering` (only first caller succeeds)
    pub(super) async fn check_and_begin_recovery(&self) -> Result<CheckResult, AppError> {
        // Fast path: read lock to check status
        let (attempting_recovery, is_impaired) = {
            let status = self.status.read().await;
            let attempting_recovery = status.check_recovery_eligibility()?;
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

                // Double-check state hasn't changed
                if matches!(*status, EndpointStatus::Offline { .. }) {
                    tracing::info!(
                        group = %self.name,
                        "ESI endpoint group beginning recovery attempt after cooldown period"
                    );
                    status.begin_recovery_attempt();
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
}

impl EndpointStatus {
    /// Check if the endpoint is offline and eligible for recovery.
    ///
    /// This method determines whether a recovery attempt should be made based on
    /// the endpoint's current status and cooldown timing.
    ///
    /// # Returns
    /// - `Ok(true)` - Endpoint is `Offline` AND cooldown has expired (eligible for recovery)
    /// - `Ok(false)` - Endpoint is not `Offline` (no recovery needed)
    /// - `Err(AppError::EsiEndpointOffline)` - Endpoint is `Offline` but cooldown hasn't expired
    ///
    /// # Note
    /// This is a read-only check. Actual state transition to `Recovering` is performed
    /// by `begin_recovery_attempt()` after this check passes.
    pub(super) fn check_recovery_eligibility(&self) -> Result<bool, AppError> {
        match self {
            EndpointStatus::Offline { last_error } => {
                let now = Utc::now();
                let elapsed = now.signed_duration_since(*last_error);

                if elapsed.to_std().unwrap_or(Duration::ZERO) < ENDPOINT_GROUP_RETRY_COOLDOWN {
                    Err(AppError::EsiEndpointOffline)
                } else {
                    // Cooldown has passed, allow retry and signal recovery attempt
                    Ok(true)
                }
            }
            _ => Ok(false),
        }
    }

    /// Transition from Offline to Recovering state.
    ///
    /// This method is called after `check_recovery_eligibility()` returns `Ok(true)`
    /// to atomically transition the endpoint into recovery mode.
    ///
    /// # Behavior
    /// - If current state is `Offline`: Transitions to `Recovering`
    /// - If current state is not `Offline`: No-op (another thread may have already transitioned)
    pub(super) fn begin_recovery_attempt(&mut self) {
        if let EndpointStatus::Offline { .. } = self {
            *self = EndpointStatus::Recovering;
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
                    *self = EndpointStatus::Offline { last_error: now };
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
                *self = EndpointStatus::Offline { last_error: now };
            }
            EndpointStatus::Offline { .. } => {}
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
            EndpointStatus::Offline { .. } => {
                // No-op, must go through recovery first
            }
        }
    }
}

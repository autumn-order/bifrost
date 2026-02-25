//! ESI endpoint group circuit breaker implementation.
//!
//! This module implements a circuit breaker pattern for ESI endpoint groups,
//! tracking error rates and automatically disabling/re-enabling endpoints based
//! on their health status. The circuit breaker has four states:
//!
//! - `Healthy`: Normal operation, no recent errors
//! - `Impaired`: Errors detected within the error window, counting towards threshold
//! - `Recovering`: Attempting to recover from Offline state after cooldown
//! - `Offline`: Circuit breaker tripped, requests blocked until cooldown expires
//!
//! The implementation uses a read-write lock for status management and an atomic
//! flag to coordinate recovery attempts across concurrent requests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;
use tokio::sync::RwLock;

use super::{
    ENDPOINT_GROUP_ERROR_LIMIT, ENDPOINT_GROUP_ERROR_WINDOW, ENDPOINT_GROUP_RETRY_COOLDOWN,
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
/// - `Impaired` → `Healthy`: Error window expires without reaching threshold
/// - `Impaired` → `Offline`: Error count reaches threshold within window
/// - `Offline` → `Recovering`: Cooldown expires and request attempts recovery
/// - `Recovering` → `Healthy`: Successful request during recovery
/// - `Recovering` → `Offline`: Too many errors during recovery (stricter than Impaired)
#[derive(Debug, Clone)]
pub(super) enum EndpointStatus {
    /// Normal operation, no recent errors
    Healthy,
    /// Errors detected, counting towards circuit breaker threshold
    Impaired {
        /// Timestamp of first error in current window (used for window expiration)
        first_error: DateTime<Utc>,
        /// Timestamp of most recent error (kept for observability/debugging)
        last_error: DateTime<Utc>,
        /// Number of errors within current window
        error_count: usize,
    },
    /// Attempting to recover from Offline state after cooldown
    Recovering {
        /// Timestamp of most recent error during recovery
        last_error: DateTime<Utc>,
        /// Number of errors since recovery began (stricter threshold than Impaired)
        error_count: usize,
    },
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

    /// Reset endpoint to healthy state if conditions are met.
    ///
    /// This method is called after a successful request to potentially reset the endpoint
    /// from `Impaired` or `Recovering` back to `Healthy`. It uses the provided `CheckResult`
    /// to optimize lock acquisition - if the endpoint was `Healthy` at check time, no locks
    /// are acquired.
    ///
    /// # Arguments
    /// - `check_result` - The result from `check_and_begin_recovery()`, indicating state at check time
    ///
    /// # Behavior
    /// - If `check_result` indicates `Healthy`: No-op, no locks acquired (fast path)
    /// - If `Recovering`: Immediately resets to `Healthy` on success
    /// - If `Impaired`: Resets to `Healthy` only if error window has expired since first error
    ///
    /// # Design Note
    /// The `check_result` parameter creates temporal coupling with `check_and_begin_recovery()`
    /// but provides a significant performance optimization by avoiding lock acquisition when
    /// the endpoint is healthy. The state is double-checked after acquiring locks to handle
    /// concurrent modifications.
    pub(super) async fn maybe_reset_to_healthy(&self, check_result: CheckResult) {
        if check_result.attempting_recovery || check_result.was_impaired {
            // Fast path: check if reset is needed with read lock
            let should_reset = {
                let status = self.status.read().await;
                status.should_reset_to_healthy()
            };

            if should_reset {
                // Only acquire write lock if we actually need to reset
                let mut status = self.status.write().await;
                let was_recovering = matches!(*status, EndpointStatus::Recovering { .. });
                let was_impaired = matches!(*status, EndpointStatus::Impaired { .. });

                // Double-check after acquiring write lock (state may have changed)
                if status.should_reset_to_healthy() {
                    status.reset_to_healthy();

                    // Log successful recovery
                    if was_recovering {
                        tracing::info!(
                            group = %self.name,
                            "ESI endpoint group successfully recovered to healthy state"
                        );
                    } else if was_impaired {
                        tracing::debug!(
                            group = %self.name,
                            "ESI endpoint group reset to healthy after error window expired"
                        );
                    }
                }

                // If we successfully recovered to Healthy, clear the recovery flag
                if was_recovering && matches!(*status, EndpointStatus::Healthy) {
                    self.recovering.store(false, Ordering::Release);
                }
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
    /// - If current state is `Offline`: Transitions to `Recovering` with error count reset
    /// - If current state is not `Offline`: No-op (another thread may have already transitioned)
    ///
    /// Preserves the `last_error` timestamp from the `Offline` state to maintain
    /// accurate error history for observability.
    pub(super) fn begin_recovery_attempt(&mut self) {
        if let EndpointStatus::Offline { last_error } = self {
            *self = EndpointStatus::Recovering {
                last_error: *last_error,
                error_count: 0,
            };
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
    /// Transitions to `Impaired` with error count of 1.
    ///
    /// ## Impaired
    /// - If error window expired: Resets to `Impaired` with new window and count of 1
    /// - If within window: Increments error count
    ///   - If count reaches threshold: Transitions to `Offline`
    ///   - Otherwise: Remains `Impaired` with updated count
    ///
    /// ## Recovering
    /// Applies stricter threshold (3 errors vs 20 for Impaired):
    /// - If count reaches 3: Transitions to `Offline`
    /// - Otherwise: Remains `Recovering` with updated count
    ///
    /// ## Offline
    /// No-op, remains `Offline` until cooldown expires.
    ///
    /// # Design Note
    /// The stricter threshold during recovery (3 errors) assumes each error will be
    /// retried multiple times at the HTTP client layer, allowing for approximately
    /// 9 total attempts (3 errors × ~3 attempts each) before giving up.
    pub(super) fn handle_5xx_error(&mut self, group_name: &str) {
        let now = Utc::now();

        match self {
            EndpointStatus::Healthy => {
                tracing::debug!(
                    group = %group_name,
                    "ESI endpoint group transitioned from Healthy to Impaired after first 5xx error"
                );
                *self = EndpointStatus::Impaired {
                    first_error: now,
                    last_error: now,
                    error_count: 1,
                };
            }
            EndpointStatus::Impaired {
                first_error,
                last_error: _,
                error_count,
            } => {
                let window_elapsed = now.signed_duration_since(*first_error);

                if window_elapsed.to_std().unwrap_or(Duration::ZERO) >= ENDPOINT_GROUP_ERROR_WINDOW
                {
                    // Window expired, start new window
                    tracing::debug!(
                        group = %group_name,
                        old_error_count = %error_count,
                        "ESI endpoint group error window expired, starting new window"
                    );
                    *self = EndpointStatus::Impaired {
                        first_error: now,
                        last_error: now,
                        error_count: 1,
                    };
                } else {
                    // Still within window, accumulate errors
                    let new_count = *error_count + 1;

                    if new_count >= ENDPOINT_GROUP_ERROR_LIMIT {
                        tracing::error!(
                            group = %group_name,
                            error_count = %new_count,
                            error_limit = %ENDPOINT_GROUP_ERROR_LIMIT,
                            window_seconds = %ENDPOINT_GROUP_ERROR_WINDOW.as_secs(),
                            cooldown_seconds = %ENDPOINT_GROUP_RETRY_COOLDOWN.as_secs(),
                            "ESI endpoint group circuit breaker tripped - too many 5xx errors within window; endpoint now offline"
                        );
                        *self = EndpointStatus::Offline { last_error: now };
                    } else {
                        *self = EndpointStatus::Impaired {
                            first_error: *first_error,
                            last_error: now,
                            error_count: new_count,
                        };
                    }
                }
            }
            EndpointStatus::Recovering { error_count, .. } => {
                // Stricter during recovery - only allow 3 errors (each with 2 retries = 9 attempts total)
                let new_count = *error_count + 1;

                if new_count >= 3 {
                    tracing::error!(
                        group = %group_name,
                        error_count = %new_count,
                        "ESI endpoint group failed recovery - too many errors during recovery attempt; returning to offline state"
                    );
                    *self = EndpointStatus::Offline { last_error: now };
                } else {
                    tracing::debug!(
                        group = %group_name,
                        error_count = %new_count,
                        "ESI endpoint group encountered error during recovery attempt"
                    );
                    *self = EndpointStatus::Recovering {
                        last_error: now,
                        error_count: new_count,
                    };
                }
            }
            EndpointStatus::Offline { .. } => {}
        }
    }

    /// Check if the status should be reset to healthy (read-only check).
    ///
    /// Determines whether conditions are met for resetting to `Healthy` without
    /// actually performing the state transition.
    ///
    /// # Returns
    /// - `true` - Conditions met for reset (caller should acquire write lock and call `reset_to_healthy()`)
    /// - `false` - Should not reset
    ///
    /// # Reset conditions
    /// - `Recovering`: Always returns true (any success during recovery resets immediately)
    /// - `Impaired`: Returns true if error window has expired since first error
    /// - `Healthy` or `Offline`: Returns false
    ///
    /// # Design Note
    /// For `Impaired` state, the error window must fully expire before resetting to `Healthy`.
    /// This means even if requests are succeeding, the endpoint remains `Impaired` until
    /// the window passes. This provides stability and prevents rapid state oscillation,
    /// but means the endpoint may remain `Impaired` longer than strictly necessary.
    fn should_reset_to_healthy(&self) -> bool {
        match self {
            EndpointStatus::Recovering { .. } => true, // Always reset on success
            EndpointStatus::Impaired { first_error, .. } => {
                let now = Utc::now();
                let elapsed = now.signed_duration_since(*first_error);
                elapsed.to_std().unwrap_or(Duration::ZERO) >= ENDPOINT_GROUP_ERROR_WINDOW
            }
            _ => false,
        }
    }

    /// Actually perform the reset to healthy (mutating operation).
    ///
    /// Transitions the endpoint status to `Healthy` if currently in a degraded state.
    /// This should only be called after `should_reset_to_healthy()` returns true and
    /// a write lock has been acquired.
    ///
    /// # Behavior
    /// - `Recovering` or `Impaired`: Transitions to `Healthy`
    /// - Other states: No-op
    fn reset_to_healthy(&mut self) {
        match self {
            EndpointStatus::Recovering { .. } | EndpointStatus::Impaired { .. } => {
                *self = EndpointStatus::Healthy;
            }
            _ => {}
        }
    }
}

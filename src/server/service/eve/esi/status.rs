//! ESI endpoint status state machine for circuit breaker and rate limit tracking.
//!
//! This module defines the `EndpointStatus` enum and its state machine logic, which is
//! used by `EndpointGroup` to track the health and availability of ESI endpoint groups.
//!
//! # Overview
//!
//! The `EndpointStatus` state machine manages two independent concerns:
//! 1. **Circuit Breaker**: Tracks 5xx errors via a sliding window to detect failing endpoints
//! 2. **Rate Limiting**: Tracks 429 responses to respect ESI rate limits
//!
//! These concerns are unified into a single state machine with five states that handle
//! both error tracking and rate limit enforcement.
//!
//! # States
//!
//! ## Healthy
//! Normal operation with no recent errors or rate limits.
//!
//! ## Impaired
//! Errors have been detected and are being tracked in a sliding window. The endpoint
//! remains operational but is being monitored. If the error rate exceeds the threshold,
//! transitions to Offline.
//!
//! ## Recovering
//! Attempting to recover from Offline state after the cooldown period expires. This state
//! applies strict fail-fast behavior - any error immediately returns to Offline, while
//! a successful request transitions to Healthy.
//!
//! ## Offline
//! Circuit breaker has tripped due to excessive errors. **ALL requests** (public and
//! authenticated) are blocked until the cooldown period expires. May preserve rate limit
//! information for proper state transition after cooldown.
//!
//! ## RateLimited
//! ESI returned 429 (Too Many Requests) with retry_after. **Only public requests** are
//! blocked - authenticated requests bypass this state entirely since they have independent
//! rate limits per access token.
//!
//! # Circuit Breaker vs Rate Limiting
//!
//! ## Circuit Breaker (5xx Errors)
//! - **Scope**: Affects ALL requests (public + authenticated)
//! - **Trigger**: Error rate exceeds threshold in sliding window
//! - **States**: Healthy → Impaired → Offline → Recovering → Healthy
//! - **Recovery**: Requires cooldown period, then successful request
//!
//! ## Rate Limiting (429 Responses)
//! - **Scope**: Affects only PUBLIC requests (unauthenticated)
//! - **Trigger**: ESI returns 429 with retry_after duration
//! - **States**: Any → RateLimited → Healthy
//! - **Recovery**: Automatic after retry_after expires
//! - **Bypass**: Authenticated requests ignore RateLimited state
//!
//! # State Priority
//!
//! When multiple conditions exist:
//! 1. **Offline** takes precedence over RateLimited (blocks all requests)
//! 2. **RateLimited** only blocks public requests
//! 3. **Recovering** applies strict error checking

use std::collections::VecDeque;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dioxus_logger::tracing;

use super::{
    ENDPOINT_GROUP_ERROR_RATE_THRESHOLD, ENDPOINT_GROUP_RETRY_COOLDOWN,
    ENDPOINT_GROUP_SLIDING_WINDOW_SIZE,
};
use crate::server::error::AppError;

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

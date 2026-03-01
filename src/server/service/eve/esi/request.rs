//! Request wrapper for ESI endpoints with circuit breaker protection.
//!
//! This module provides `EsiProviderRequest`, a wrapper around `eve_esi::EsiRequest`
//! that adds automatic circuit breaker protection. It supports both standard and
//! cached request patterns.

use std::sync::Arc;

use dioxus_logger::tracing;
use eve_esi::{CacheStrategy, CachedResponse, EsiResponse};

use super::group::EndpointGroup;
use crate::server::error::AppError;

/// Wrapper for ESI requests that adds circuit breaker protection.
///
/// This struct wraps an `EsiRequest` from the `eve_esi` crate and adds automatic
/// circuit breaker logic. It supports both standard requests (`.send()`) and
/// cached requests (`.send_cached()`) while tracking endpoint health.
///
/// # Type Parameters
/// - `T` - The response data type expected from the ESI endpoint
///
/// # Circuit Breaker Behavior
/// Both `send()` and `send_cached()` methods:
/// - Check circuit breaker status before making the request
/// - Track 5xx errors and update circuit breaker state
/// - Reset to healthy on successful responses
/// - Return `AppError::EsiEndpointOffline` when circuit breaker is open
pub struct EsiProviderRequest<'a, T> {
    /// Reference to the endpoint group's circuit breaker state
    group: &'a Arc<EndpointGroup>,
    /// The underlying ESI request from eve_esi crate
    request: eve_esi::EsiRequest<T>,
}

impl<'a, T> EsiProviderRequest<'a, T>
where
    T: serde::de::DeserializeOwned,
{
    /// Creates a new ESI provider request with circuit breaker protection.
    ///
    /// # Arguments
    /// - `group` - Reference to the endpoint group's circuit breaker state
    /// - `request` - The underlying ESI request to wrap
    ///
    /// # Returns
    /// New `EsiProviderRequest` instance ready to be sent
    pub fn new(group: &'a Arc<EndpointGroup>, request: eve_esi::EsiRequest<T>) -> Self {
        Self { group, request }
    }

    /// Sends the ESI request expecting a fresh response.
    ///
    /// This method wraps the standard ESI request flow with circuit breaker protection.
    /// It expects a 200 OK response with data.
    ///
    /// # Circuit Breaker Flow
    /// 1. Checks circuit breaker status (returns error if offline)
    /// 2. Sends the ESI request
    /// 3. On 5xx error: Updates circuit breaker state
    /// 4. On success: Resets circuit breaker to healthy if recovering
    ///
    /// # Returns
    /// - `Ok(EsiResponse<T>)` - Successful response with data and cache headers
    /// - `Err(AppError::EsiEndpointOffline)` - Circuit breaker is open
    /// - `Err(AppError::Esi)` - ESI request failed
    /// - `Err(AppError)` - Other errors (network, parsing, etc.)
    pub async fn send(self) -> Result<EsiResponse<T>, AppError> {
        let has_access_token = self.request.access_token().is_some();

        // Check status and atomically begin recovery if needed
        let check_result = self
            .group
            .check_and_begin_recovery(has_access_token)
            .await?;

        tracing::trace!(
            attempting_recovery = %check_result.attempting_recovery,
            was_impaired = %check_result.was_impaired,
            "Executing ESI request"
        );

        let result = self.request.send().await;

        match &result {
            Err(eve_esi::Error::EsiError(err)) => {
                self.group.handle_esi_error(err, has_access_token).await;
            }
            Ok(_) => {
                tracing::trace!("ESI request successful");
                self.group.handle_success(check_result).await;
            }
            _ => {}
        }

        result.map_err(Into::into)
    }

    /// Sends the ESI request with cache strategy support.
    ///
    /// This method wraps the cached ESI request flow with circuit breaker protection.
    /// It supports conditional requests (If-Modified-Since) and can return either
    /// fresh data or a "not modified" response.
    ///
    /// # Arguments
    /// - `strategy` - Cache strategy (e.g., `CacheStrategy::IfModifiedSince(datetime)`)
    ///
    /// # Circuit Breaker Flow
    /// 1. Checks circuit breaker status (returns error if offline)
    /// 2. Sends the ESI request with cache headers
    /// 3. On 5xx error: Updates circuit breaker state
    /// 4. On success (200 or 304): Resets circuit breaker to healthy if recovering
    ///
    /// # Returns
    /// - `Ok(CachedResponse::Fresh(EsiResponse<T>))` - New data returned (200 OK)
    /// - `Ok(CachedResponse::NotModified)` - Data unchanged (304 Not Modified)
    /// - `Err(AppError::EsiEndpointOffline)` - Circuit breaker is open
    /// - `Err(AppError::Esi)` - ESI request failed
    /// - `Err(AppError)` - Other errors (network, parsing, etc.)
    pub async fn send_cached(
        self,
        strategy: CacheStrategy,
    ) -> Result<CachedResponse<EsiResponse<T>>, AppError> {
        let has_access_token = self.request.access_token().is_some();

        // Check status and atomically begin recovery if needed
        let check_result = self
            .group
            .check_and_begin_recovery(has_access_token)
            .await?;

        tracing::trace!(
            attempting_recovery = %check_result.attempting_recovery,
            was_impaired = %check_result.was_impaired,
            "Executing cached ESI request"
        );

        let result = self.request.send_cached(strategy).await;

        match &result {
            Err(eve_esi::Error::EsiError(err)) => {
                self.group.handle_esi_error(err, has_access_token).await;
            }
            Ok(CachedResponse::Fresh(_)) => {
                tracing::trace!("Cached ESI request returned fresh data (200 OK)");
                // Both Fresh and NotModified are considered successful responses
                self.group.handle_success(check_result).await;
            }
            Ok(CachedResponse::NotModified) => {
                tracing::trace!("Cached ESI request returned 304 Not Modified");
                // Both Fresh and NotModified are considered successful responses
                self.group.handle_success(check_result).await;
            }
            _ => {}
        }

        result.map_err(Into::into)
    }
}

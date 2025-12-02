//! EVE Online-specific error types.
//!
//! This module defines errors specific to EVE Online data integration, particularly related
//! to ESI data inconsistencies or missing reference data. These errors typically indicate
//! edge cases or data synchronization issues between Bifrost and EVE's ESI.

use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

/// EVE Online data integration error type.
///
/// These errors occur when Bifrost encounters unexpected issues with EVE Online reference
/// data, such as missing faction information. Most of these errors are edge cases that may
/// resolve themselves when ESI cache updates, but they are logged and reported to help
/// identify potential bugs or ESI inconsistencies.
#[derive(Error, Debug)]
pub enum EveError {
    /// NPC faction information not found in local database.
    ///
    /// This error occurs when a character, corporation, or alliance references a faction ID
    /// that doesn't exist in Bifrost's faction table. This should never happen under normal
    /// circumstances, as factions are static NPC entities that are pre-loaded from ESI.
    ///
    /// # Cause
    /// This could indicate:
    /// - ESI returned a new faction ID that hasn't been synced yet
    /// - Database migration or seeding issue
    /// - ESI data inconsistency
    ///
    /// # Resolution
    /// The error is non-fatal and typically resolves itself within 24 hours when the ESI
    /// faction cache updates and Bifrost's scheduler refreshes faction data. Until resolved,
    /// the affected entities will not show faction membership.
    ///
    /// # Reporting
    /// If this error persists, it should be reported as a GitHub issue at:
    /// https://github.com/autumn-order/bifrost
    #[error(
        "Failed to find information for EVE Online NPC faction ID: {0:?}\n\
        \n\
        This should never occur but if it has please open a GitHub issue so we can look into it:\n\
        https://github.com/autumn-order/bifrost\n\
        \n\
        For now, this faction will not be saved to the database and no characters, corporations, or alliances
        will show membership to this faction until the issue is resolved."
    )]
    FactionNotFound(i64),
}

/// Converts EVE Online errors into HTTP responses.
///
/// All EVE errors are treated as internal server errors (500) since they indicate data
/// synchronization issues or unexpected ESI behavior rather than client errors. The full
/// error message is logged for debugging.
///
/// # Returns
/// A 500 Internal Server Error response with a generic error message
impl IntoResponse for EveError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}

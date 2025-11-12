use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::server::error::InternalServerError;

#[derive(Error, Debug)]
pub enum EveError {
    // This is an edge case error, it'll probably end up resolving itself after 24 hours should it occur
    // once the EVE Online ESI faction cache updates.
    //
    // This issue is not fatal, the affected characters/corporations/alliances
    // can safely be set to not be a member of this faction as a temporary solution
    // until the issue is resolved.
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

impl IntoResponse for EveError {
    fn into_response(self) -> Response {
        InternalServerError(self).into_response()
    }
}

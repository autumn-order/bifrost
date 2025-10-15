use tower_sessions::Session;

use crate::server::{error::Error, model::session::AuthLoginCsrf};

/// Validate that the session CSRF state exists and matches `state`.
/// Returns `Ok(())` when valid or the appropriate `Error` otherwise.
pub async fn validate_csrf(session: &Session, csrf_state: &str) -> Result<(), Error> {
    let stored_state = AuthLoginCsrf::get(session).await?;
    if stored_state != csrf_state {
        Err(Error::AuthCsrfInvalidState)
    } else {
        Ok(())
    }
}

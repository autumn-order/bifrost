//! Tests for the logout endpoint.
//!
//! This module verifies the logout endpoint's behavior, including successful
//! session cleanup when a user logs out and safe handling of logout requests
//! when no session data exists.

use axum::{http::StatusCode, response::IntoResponse};
use bifrost::server::{controller::auth::logout, model::session::user::SessionUserId};

use super::*;

/// Tests successful logout with user session cleanup.
///
/// Verifies that the logout endpoint successfully clears the user ID from the
/// session and returns a 307 temporary redirect to the login page when a logged-in
/// user initiates logout.
///
/// Expected: Ok with 307 TEMPORARY_REDIRECT response and session cleared
#[tokio::test]
async fn logs_out_user_successfully() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let user_id = 1;
    SessionUserId::insert(&test.session, user_id).await.unwrap();

    let result = logout(test.session.clone()).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    // Ensure user was cleared from session
    let maybe_user_id = SessionUserId::get(&test.session).await.unwrap();
    assert!(maybe_user_id.is_none());

    Ok(())
}

/// Tests logout behavior when no session data exists.
///
/// Verifies that the logout endpoint safely handles the case where there is no
/// user session data, avoiding internal errors by only clearing session when data
/// exists. The endpoint redirects to login regardless of session state.
///
/// Expected: Ok with 307 TEMPORARY_REDIRECT response
#[tokio::test]
async fn redirects_when_no_session_data() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let result = logout(test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    Ok(())
}

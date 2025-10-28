use axum::{http::StatusCode, response::IntoResponse};
use bifrost::server::{
    controller::auth::logout, error::Error, model::session::user::SessionUserId,
};

use crate::util::setup::test_setup;

#[tokio::test]
/// Expect 307 temporary redirect after logout with a user ID in session
async fn returns_redirect_on_logout_with_user_id() -> Result<(), Error> {
    let test = test_setup().await;

    let user_id = 1;
    SessionUserId::insert(&test.session, user_id).await?;

    let result = logout(test.session.clone()).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    // Ensure user was cleared from session
    let maybe_user_id = SessionUserId::get(&test.session).await?;
    assert!(maybe_user_id.is_none());

    Ok(())
}

#[tokio::test]
/// Expect 307 temporary redirect after logout even without session data
///
/// This checks for the 500 internal error that occurs when clearing
/// a session without any data in it. To resolve this, the endpoint doesn't
/// clear session unless there is actually a user ID in session, it will redirect
/// to login regardless of clear being called.
async fn returns_redirect_on_logout_with_no_session() -> Result<(), Error> {
    let test = test_setup().await;

    let result = logout(test.session).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);

    Ok(())
}

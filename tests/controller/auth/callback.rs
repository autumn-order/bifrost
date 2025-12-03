//! Tests for the OAuth callback endpoint.
//!
//! This module verifies the OAuth callback endpoint's behavior during the EVE Online
//! SSO authentication flow, including successful authentication for new and existing
//! users, CSRF validation, and error handling when ESI is unavailable.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::{
    controller::auth::{callback, CallbackParams},
    model::session::{auth::SessionAuthCsrf, user::SessionUserId},
};

use super::*;

/// Tests successful redirect after OAuth callback for new user.
///
/// Verifies that when a new character logs in via EVE SSO, the callback endpoint
/// successfully processes the OAuth code, creates the user and character records,
/// stores the user ID in session, and redirects to the home page.
///
/// Expected: Ok with 308 PERMANENT_REDIRECT response and user ID in session
#[tokio::test]
async fn redirects_for_new_user() -> Result<(), TestError> {
    let corporation_id = 1;
    let character_id = 1;
    let mock_corporation = factory::mock_corporation(None, None);
    let mock_character = factory::mock_character(corporation_id, None, None);

    let test = TestBuilder::new()
        .with_user_tables()
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, "owner_hash")
        .build()
        .await?;

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    let result = callback(
        State(test.into_app_state()),
        test.session.clone(),
        Query(params),
    )
    .await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);

    // Assert user is in session
    let result = SessionUserId::get(&test.session).await;
    assert!(result.is_ok());
    let maybe_user_id = result.unwrap();
    assert!(maybe_user_id.is_some());
    let user_id = maybe_user_id.unwrap();

    // User ID should be 1 as it would be the first user created in database
    assert_eq!(user_id, 1);

    test.assert_mocks();

    Ok(())
}

/// Tests successful redirect after OAuth callback for existing user.
///
/// Verifies that when an existing user logs in via EVE SSO, the callback endpoint
/// successfully validates the JWT token, updates the session, and redirects to the
/// home page without creating duplicate records.
///
/// Expected: Ok with 308 PERMANENT_REDIRECT response and existing user ID in session
#[tokio::test]
async fn redirects_for_existing_user() -> Result<(), TestError> {
    let mut test = TestBuilder::new().with_user_tables().build().await?;

    let (user_model, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let jwt_endpoints = test.auth().create_jwt_endpoints(
        character_model.character_id,
        &user_character_model.owner_hash,
    );

    SessionUserId::insert(&test.session, user_model.id)
        .await
        .unwrap();
    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    let result = callback(
        State(test.into_app_state()),
        test.session.clone(),
        Query(params),
    )
    .await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);

    // Assert user is in session
    let result = SessionUserId::get(&test.session).await;
    assert!(result.is_ok());
    let maybe_user_id = result.unwrap();
    assert!(maybe_user_id.is_some());
    let user_id = maybe_user_id.unwrap();

    // User ID should be 1 as it would be the first user created in database
    assert_eq!(user_id, 1);

    // Assert JWT keys & token were fetched during callback
    for endpoint in jwt_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Tests CSRF protection validation during OAuth callback.
///
/// Verifies that the callback endpoint rejects requests when the CSRF state
/// parameter doesn't match the value stored in session, preventing CSRF attacks
/// on the authentication flow.
///
/// Expected: Err with 400 BAD_REQUEST response
#[tokio::test]
async fn fails_for_invalid_csrf_state() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let mut params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    params.state = "incorrect_state".to_string();

    let result = callback(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

/// Tests error handling when ESI is unavailable during callback.
///
/// Verifies that the callback endpoint returns a 500 internal server error when
/// it cannot communicate with the ESI API to complete the OAuth token exchange,
/// indicating a service dependency failure.
///
/// Expected: Err with 500 INTERNAL_SERVER_ERROR response
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();
    let result = callback(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

/// Tests that change_main flag from session is used during callback.
///
/// Verifies that when the change_main flag is set in session (from login),
/// the callback endpoint retrieves it and uses it to update the user's main
/// character after successful authentication.
///
/// Expected: Ok with 308 PERMANENT_REDIRECT and main character updated
#[tokio::test]
async fn uses_change_main_flag_from_session() -> Result<(), TestError> {
    use bifrost::server::model::session::change_main::SessionUserChangeMain;

    let character_id_1 = 111111111;
    let character_id_2 = 222222222;
    let owner_hash = "owner_hash_123";

    let mut test = TestBuilder::new().with_user_tables().build().await?;

    // Create user with first character as main
    let (user, _, _) = test
        .user()
        .insert_user_with_mock_character(character_id_1, 1, None, None)
        .await?;

    // Add second character to same user
    let (ownership2, char2) = test
        .user()
        .insert_mock_character_for_user(user.id, character_id_2, 2, None, None)
        .await?;

    // Update owner hash for second character
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    entity::bifrost_user_character::Entity::update_many()
        .col_expr(
            entity::bifrost_user_character::Column::OwnerHash,
            sea_orm::sea_query::Expr::value(owner_hash),
        )
        .filter(entity::bifrost_user_character::Column::Id.eq(ownership2.id))
        .exec(&test.db)
        .await?;

    // Set up session with user logged in and change_main flag
    SessionUserId::insert(&test.session, user.id).await.unwrap();
    SessionUserChangeMain::insert(&test.session, true)
        .await
        .unwrap();

    let jwt_endpoints = test.auth().create_jwt_endpoints(character_id_2, owner_hash);

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    let result = callback(
        State(test.into_app_state()),
        test.session.clone(),
        Query(params),
    )
    .await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);

    // Verify main character was updated to second character
    use bifrost::server::data::user::UserRepository;
    let user_repo = UserRepository::new(&test.db);
    let (updated_user, _) = user_repo.get_by_id(user.id).await?.unwrap();
    assert_eq!(updated_user.main_character_id, char2.id);

    // Verify change_main flag was removed from session
    let change_main = SessionUserChangeMain::remove(&test.session).await.unwrap();
    assert_eq!(change_main, None);

    for endpoint in jwt_endpoints {
        endpoint.assert();
    }

    Ok(())
}

/// Tests that user ID is not re-inserted when already logged in.
///
/// Verifies that when a user who is already logged in (has user_id in session)
/// adds a new character, the session user_id is not unnecessarily updated,
/// maintaining session integrity.
///
/// Expected: Ok with 308 PERMANENT_REDIRECT and same user ID in session
#[tokio::test]
async fn does_not_reinsert_user_id_when_already_logged_in() -> Result<(), TestError> {
    let character_id = 123456789;
    let corporation_id = 1;
    let owner_hash = "owner_hash_123";

    let mock_corporation = factory::mock_corporation(None, None);
    let mock_character = factory::mock_character(corporation_id, None, None);

    let mut test = TestBuilder::new()
        .with_user_tables()
        .with_corporation_endpoint(corporation_id, mock_corporation, 1)
        .with_character_endpoint(character_id, mock_character, 1)
        .with_jwt_endpoints(character_id, owner_hash)
        .build()
        .await?;

    // Create existing user
    let (user, _, _) = test
        .user()
        .insert_user_with_mock_character(987654321, 2, None, None)
        .await?;

    // Set user in session
    SessionUserId::insert(&test.session, user.id).await.unwrap();

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };
    SessionAuthCsrf::insert(&test.session, &params.state)
        .await
        .unwrap();

    let result = callback(
        State(test.into_app_state()),
        test.session.clone(),
        Query(params),
    )
    .await;

    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);

    // Verify user ID is still the same in session
    let session_user_id = SessionUserId::get(&test.session).await.unwrap().unwrap();
    assert_eq!(session_user_id, user.id);

    test.assert_mocks();

    Ok(())
}

/// Tests CSRF validation failure when no token in session.
///
/// Verifies that the callback endpoint rejects requests when no CSRF token
/// was ever stored in the session, preventing unauthorized callback attempts.
///
/// Expected: Err with 500 INTERNAL_SERVER_ERROR response
#[tokio::test]
async fn fails_when_no_csrf_token_in_session() -> Result<(), TestError> {
    let test = TestBuilder::new().with_user_tables().build().await?;

    let params = CallbackParams {
        state: "state".to_string(),
        code: "code".to_string(),
    };

    // Don't insert CSRF token in session

    let result = callback(State(test.into_app_state()), test.session, Query(params)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

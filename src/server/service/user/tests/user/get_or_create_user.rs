use eve_esi::model::oauth2::EveJwtClaims;

use crate::server::{
    data::user::user_character::UserCharacterRepository, error::Error, service::user::UserService,
};

use super::*;

/// Expect Ok when user associated with character is found
#[tokio::test]
async fn finds_existing_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    // Set character ID in claims to the mock character
    let mut claims = EveJwtClaims::mock();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
    claims.owner = user_character_model.owner_hash;

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.get_or_create_user(claims).await;

    assert!(result.is_ok());
    let user_id = result.unwrap();
    assert_eq!(user_id, user_model.id);

    Ok(())
}

/// Expect Ok & character transfer if owner hash for character has changed, requiring a new user
#[tokio::test]
async fn transfers_character_on_owner_hash_change() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (_, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let mut claims = EveJwtClaims::mock();
    claims.sub = "CHARACTER:EVE:1".to_string();
    claims.owner = "different_owner_hash".to_string();

    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.get_or_create_user(claims).await;

    assert!(result.is_ok());
    // Ensure character was actually transferred & new user created
    let user_character_result = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_user_character_model) = user_character_result.unwrap();
    let updated_user_character_model = maybe_user_character_model.unwrap();

    assert_ne!(
        updated_user_character_model.user_id,
        user_character_model.user_id
    );

    Ok(())
}

/// Expect Ok when character is found but new user is created
#[tokio::test]
async fn creates_user_for_existing_character() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    // Set character ID in claims to the mock character
    let mut claims = EveJwtClaims::mock();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.get_or_create_user(claims).await;

    assert!(result.is_ok());

    Ok(())
}

/// Expect Ok when new character & user is created
#[tokio::test]
async fn creates_user_and_character() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;

    let corporation_id = 1;
    let (_, mock_corporation) = test.eve().with_mock_corporation(corporation_id, None, None);
    let (character_id, mock_character) =
        test.eve()
            .with_mock_character(1, corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);
    let character_endpoint = test
        .eve()
        .with_character_endpoint(character_id, mock_character, 1);

    // Set character ID in claims to the mock character
    let mut claims = EveJwtClaims::mock();
    claims.sub = format!("CHARACTER:EVE:{}", character_id);

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.get_or_create_user(claims).await;

    assert!(result.is_ok());
    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect Error when the required database tables haven't been created
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    let test = test_setup_with_tables!()?;

    // Set character ID in claims to the mock character
    let mut claims = EveJwtClaims::mock();
    claims.sub = format!("CHARACTER:EVE:{}", 1);

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.get_or_create_user(claims).await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect Error when required ESI endpoints are unavailable
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    // Set character ID in claims to the mock character
    let mut claims = EveJwtClaims::mock();
    claims.sub = format!("CHARACTER:EVE:{}", 1);

    let user_service = UserService::new(&test.state.db, &test.state.esi_client);
    let result = user_service.get_or_create_user(claims).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

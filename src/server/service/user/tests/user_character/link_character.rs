use crate::server::{
    data::user::user_character::UserCharacterRepository, error::Error,
    service::user::user_character::UserCharacterService,
};

use super::*;

/// Expect no link created when finding character owned by provided user ID
#[tokio::test]
async fn skips_link_when_already_owned() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
    claims.owner = user_character_model.owner_hash;

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .link_character(user_model.id, claims)
        .await;

    assert!(result.is_ok());
    let link_created = result.unwrap();
    assert!(!link_created);

    Ok(())
}

/// Expect Ok & character transfer if owner hash hasn't changed but user ID is different
#[tokio::test]
async fn transfers_character_to_different_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (_, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let new_user_model = test
        .user()
        .insert_user(user_character_model.character_id)
        .await?;

    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
    claims.owner = user_character_model.owner_hash;

    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .link_character(new_user_model.id, claims)
        .await;

    assert!(result.is_ok());
    let link_created = result.unwrap();
    assert!(link_created);

    // Ensure character was actually transferred
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let character_ownership = maybe_ownership.unwrap();
    assert_eq!(character_ownership.user_id, new_user_model.id);

    Ok(())
}

/// Expect Ok & character transfer if ownerhash for character has changed, requiring a new user
#[tokio::test]
async fn transfers_character_on_owner_hash_change() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (_, user_character_model, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let new_user_model = test
        .user()
        .insert_user(user_character_model.character_id)
        .await?;

    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);
    claims.owner = format!("different_{}", user_character_model.owner_hash);

    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .link_character(new_user_model.id, claims)
        .await;

    assert!(result.is_ok());
    let link_created = result.unwrap();
    assert!(link_created);

    // Ensure character was actually transferred
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let character_ownership = maybe_ownership.unwrap();
    assert_eq!(character_ownership.user_id, new_user_model.id);

    Ok(())
}

/// Expect link created when character is created but not owned and linked to provided user ID
#[tokio::test]
async fn links_unowned_character() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    // Character is set as main but there isn't actually an ownership record set
    let user_model = test.user().insert_user(character_model.id).await?;

    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);

    let user_character_repo = UserCharacterRepository::new(&test.state.db);
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .link_character(user_model.id, claims)
        .await;

    assert!(result.is_ok());
    let link_created = result.unwrap();
    assert!(link_created);

    // Ensure character was actually linked
    let ownership_entry = user_character_repo
        .get_by_character_id(character_model.character_id)
        .await?;
    let (_, maybe_ownership) = ownership_entry.unwrap();
    let character_ownership = maybe_ownership.unwrap();
    assert_eq!(character_ownership.user_id, user_model.id);

    Ok(())
}

/// Expect link created when creating a new character and linking to provided user ID
#[tokio::test]
async fn creates_and_links_character() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let corporation_id = 2;
    let (_, mock_corporation) = test.eve().with_mock_corporation(corporation_id, None, None);
    let (second_character_id, mock_character) =
        test.eve()
            .with_mock_character(2, corporation_id, None, None);

    let corporation_endpoint =
        test.eve()
            .with_corporation_endpoint(corporation_id, mock_corporation, 1);
    let character_endpoint =
        test.eve()
            .with_character_endpoint(second_character_id, mock_character, 1);

    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", second_character_id);

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .link_character(user_model.id, claims)
        .await;

    assert!(result.is_ok());
    let link_created = result.unwrap();
    assert!(link_created);

    corporation_endpoint.assert();
    character_endpoint.assert();

    Ok(())
}

/// Expect database Error when user ID provided does not exist in database
#[tokio::test]
async fn fails_for_nonexistent_user() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;

    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", character_model.character_id);

    let nonexistent_id = 1;
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .link_character(nonexistent_id, claims)
        .await;

    assert!(matches!(result, Err(Error::DbErr(_))));

    Ok(())
}

/// Expect ESI error when endpoints required to create a character are not available
#[tokio::test]
async fn fails_when_esi_unavailable() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;

    let character_id = 1;
    let mut claims = test.auth().with_mock_jwt_claims();
    claims.sub = format!("CHARACTER:EVE:{}", character_id);

    let user_id = 1;
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service.link_character(user_id, claims).await;

    assert!(matches!(result, Err(Error::EsiError(_))));

    Ok(())
}

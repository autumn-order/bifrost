use crate::server::service::user::user_character::UserCharacterService;

use super::*;

/// Expect Ok with Vec containing single character DTO without alliance
#[tokio::test]
async fn returns_character_without_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let character_dto = &character_dtos[0];
    assert_eq!(character_dto.id, character_model.character_id);
    assert_eq!(character_dto.name, character_model.name);
    assert_eq!(character_dto.corporation.id, 1);
    assert!(character_dto.alliance.is_none());

    Ok(())
}

/// Expect Ok with Vec containing character DTO with alliance
#[tokio::test]
async fn returns_character_with_alliance() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;

    let alliance_model = test.eve().insert_mock_alliance(1, None).await?;
    let corporation_model = test.eve().insert_mock_corporation(1, Some(1), None).await?;
    let (user_model, _, character_model) = test
        .user()
        .insert_user_with_mock_character(
            1,
            corporation_model.corporation_id,
            Some(alliance_model.alliance_id),
            None,
        )
        .await?;

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let character_dto = &character_dtos[0];
    assert_eq!(character_dto.id, character_model.character_id);
    assert_eq!(character_dto.name, character_model.name);
    assert_eq!(character_dto.corporation.id, 1);
    assert!(character_dto.alliance.is_some());
    let alliance_dto = character_dto.alliance.as_ref().unwrap();
    assert_eq!(alliance_dto.id, alliance_model.alliance_id);
    assert_eq!(alliance_dto.name, alliance_model.name);

    Ok(())
}

/// Expect Ok with Vec containing multiple character DTOs
#[tokio::test]
async fn returns_multiple_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, character_model_1) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;
    let (_, character_model_2) = test
        .user()
        .insert_mock_character_for_user(user_model.id, 2, 2, Some(1), None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 2);

    // Validate both characters are present
    let char_ids: Vec<i64> = character_dtos.iter().map(|dto| dto.id).collect();
    assert!(char_ids.contains(&character_model_1.character_id));
    assert!(char_ids.contains(&character_model_2.character_id));

    // Validate all have corporations
    for character_dto in &character_dtos {
        assert!(character_dto.corporation.id > 0);
        assert!(!character_dto.corporation.name.is_empty());
    }

    Ok(())
}

/// Expect Ok with empty Vec when user has no characters
#[tokio::test]
async fn returns_empty_for_user_without_characters() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let character_model = test.eve().insert_mock_character(1, 1, None, None).await?;
    let user_model = test.user().insert_user(character_model.id).await?;

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 0);

    Ok(())
}

/// Expect Ok with empty Vec when requesting a nonexistent user ID
#[tokio::test]
async fn returns_empty_for_nonexistent_user() -> Result<(), TestError> {
    let test = test_setup_with_user_tables!()?;

    let nonexistent_user_id = 1;
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(nonexistent_user_id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 0);

    Ok(())
}

/// Expect DTOs to have correct timestamp fields
#[tokio::test]
async fn returns_characters_with_timestamps() -> Result<(), TestError> {
    let mut test = test_setup_with_user_tables!()?;
    let (user_model, _, _) = test
        .user()
        .insert_user_with_mock_character(1, 1, None, None)
        .await?;

    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(user_model.id)
        .await;

    assert!(result.is_ok());
    let character_dtos = result.unwrap();
    assert_eq!(character_dtos.len(), 1);

    let character_dto = &character_dtos[0];

    let now = chrono::Utc::now().naive_utc();
    let tolerance = chrono::Duration::seconds(5); // Allow 5 second window

    // Ensure timestamp are populated and recent
    assert!(
        character_dto.info_updated_at <= now && character_dto.info_updated_at >= now - tolerance,
        "Character info_updated_at should be recent"
    );
    assert!(
        character_dto.affiliation_updated_at <= now
            && character_dto.affiliation_updated_at >= now - tolerance,
        "Character affiliation_updated_at should be recent"
    );
    assert!(
        character_dto.corporation.info_updated_at <= now
            && character_dto.corporation.info_updated_at >= now - tolerance,
        "Corporation info_updated_at should be recent"
    );
    assert!(
        character_dto.corporation.affiliation_updated_at <= now
            && character_dto.corporation.affiliation_updated_at >= now - tolerance,
        "Corporation affiliation_updated_at should be recent"
    );

    Ok(())
}

/// Expect Error when required database tables do not exist
#[tokio::test]
async fn fails_when_tables_missing() -> Result<(), TestError> {
    // Use test setup that doesn't setup required tables, causing a database error
    let test = test_setup_with_tables!()?;

    let nonexistent_user_id = 1;
    let user_character_service = UserCharacterService::new(&test.state.db, &test.state.esi_client);
    let result = user_character_service
        .get_user_characters(nonexistent_user_id)
        .await;

    assert!(result.is_err());

    Ok(())
}

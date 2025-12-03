//! Tests for CallbackService::determine_character_action method.

use bifrost_test_utils::{
    auth_factory::mock_jwt_claims,
    user_factory::{mock_character_model, mock_ownership_model},
};

use super::*;

/// Tests character not found with no logged in user.
///
/// Verifies that when a character doesn't exist in the database and no user
/// is logged in, the action is to fetch from ESI and link to a new user.
///
/// Expected: FetchAndLink with to_user_id = None
#[test]
fn character_not_found_not_logged_in() {
    let session = Session::NotLoggedIn;
    let record = CharacterRecord::NotFound;
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::FetchAndLink {
            to_user_id,
            owner_hash,
        } => {
            assert_eq!(to_user_id, None);
            assert_eq!(owner_hash, "owner_hash_123");
        }
        _ => panic!("Expected FetchAndLink action, got: {:?}", action),
    }
}

/// Tests character not found with logged in user.
///
/// Verifies that when a character doesn't exist in the database and a user
/// is logged in, the action is to fetch from ESI and link to that user.
///
/// Expected: FetchAndLink with to_user_id = Some(user_id)
#[test]
fn character_not_found_logged_in() {
    let session = Session::LoggedIn(42);
    let record = CharacterRecord::NotFound;
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::FetchAndLink {
            to_user_id,
            owner_hash,
        } => {
            assert_eq!(to_user_id, Some(42));
            assert_eq!(owner_hash, "owner_hash_123");
        }
        _ => panic!("Expected FetchAndLink action, got: {:?}", action),
    }
}

/// Tests unowned character with no logged in user.
///
/// Verifies that when a character exists but is not owned by any user, and
/// no user is logged in, the action is to link to a new user.
///
/// Expected: LinkUnownedToUser with to_user_id = None
#[test]
fn unowned_character_not_logged_in() {
    let session = Session::NotLoggedIn;
    let character = mock_character_model(123456789);
    let record = CharacterRecord::Unowned {
        character: character.clone(),
    };
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::LinkUnownedToUser {
            to_user_id,
            character: returned_char,
            owner_hash,
        } => {
            assert_eq!(to_user_id, None);
            assert_eq!(returned_char.id, character.id);
            assert_eq!(owner_hash, "owner_hash_123");
        }
        _ => panic!("Expected LinkUnownedToUser action, got: {:?}", action),
    }
}

/// Tests unowned character with logged in user.
///
/// Verifies that when a character exists but is not owned by any user, and
/// a user is logged in, the action is to link to that user.
///
/// Expected: LinkUnownedToUser with to_user_id = Some(user_id)
#[test]
fn unowned_character_logged_in() {
    let session = Session::LoggedIn(42);
    let character = mock_character_model(123456789);
    let record = CharacterRecord::Unowned {
        character: character.clone(),
    };
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::LinkUnownedToUser {
            to_user_id,
            character: returned_char,
            owner_hash,
        } => {
            assert_eq!(to_user_id, Some(42));
            assert_eq!(returned_char.id, character.id);
            assert_eq!(owner_hash, "owner_hash_123");
        }
        _ => panic!("Expected LinkUnownedToUser action, got: {:?}", action),
    }
}

/// Tests owned character with no logged in user and matching owner hash.
///
/// Verifies that when a character is owned, no user is logged in, and the
/// owner hash matches, the action is to recognize already owned status.
///
/// Expected: AlreadyOwned
#[test]
fn owned_character_not_logged_in_hash_matches() {
    let session = Session::NotLoggedIn;
    let character = mock_character_model(123456789);
    let ownership = mock_ownership_model(10, character.id, "owner_hash_123");
    let record = CharacterRecord::Owned {
        character: character.clone(),
        ownership: ownership.clone(),
    };
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::AlreadyOwned {
            user_id,
            ownership: returned_ownership,
        } => {
            assert_eq!(user_id, 10);
            assert_eq!(returned_ownership.id, ownership.id);
        }
        _ => panic!("Expected AlreadyOwned action, got: {:?}", action),
    }
}

/// Tests owned character with no logged in user and different owner hash.
///
/// Verifies that when a character is owned, no user is logged in, and the
/// owner hash differs (character was transferred), the action is to transfer
/// ownership to a new user.
///
/// Expected: TransferOwnership with to_user_id = None
#[test]
fn owned_character_not_logged_in_hash_differs() {
    let session = Session::NotLoggedIn;
    let character = mock_character_model(123456789);
    let ownership = mock_ownership_model(10, character.id, "old_owner_hash");
    let record = CharacterRecord::Owned {
        character: character.clone(),
        ownership,
    };
    let claims = mock_jwt_claims(123456789, "new_owner_hash");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::TransferOwnership {
            to_user_id,
            character: returned_char,
            owner_hash,
        } => {
            assert_eq!(to_user_id, None);
            assert_eq!(returned_char.id, character.id);
            assert_eq!(owner_hash, "new_owner_hash");
        }
        _ => panic!("Expected TransferOwnership action, got: {:?}", action),
    }
}

/// Tests owned character with same user logged in and matching owner hash.
///
/// Verifies that when a character is owned by the logged-in user and the
/// owner hash matches, the action is to recognize already owned status.
///
/// Expected: AlreadyOwned
#[test]
fn owned_character_same_user_hash_matches() {
    let session = Session::LoggedIn(10);
    let character = mock_character_model(123456789);
    let ownership = mock_ownership_model(10, character.id, "owner_hash_123");
    let record = CharacterRecord::Owned {
        character: character.clone(),
        ownership: ownership.clone(),
    };
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::AlreadyOwned {
            user_id,
            ownership: returned_ownership,
        } => {
            assert_eq!(user_id, 10);
            assert_eq!(returned_ownership.id, ownership.id);
        }
        _ => panic!("Expected AlreadyOwned action, got: {:?}", action),
    }
}

/// Tests owned character with same user logged in and different owner hash.
///
/// Verifies that when a character is owned by the logged-in user but the
/// owner hash has changed (same user moved EVE accounts), the action is to
/// update only the owner hash.
///
/// Expected: UpdateOwnerHash
#[test]
fn owned_character_same_user_hash_differs() {
    let session = Session::LoggedIn(10);
    let character = mock_character_model(123456789);
    let ownership = mock_ownership_model(10, character.id, "old_owner_hash");
    let record = CharacterRecord::Owned {
        character: character.clone(),
        ownership,
    };
    let claims = mock_jwt_claims(123456789, "new_owner_hash");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::UpdateOwnerHash {
            user_id,
            character: returned_char,
            owner_hash,
        } => {
            assert_eq!(user_id, 10);
            assert_eq!(returned_char.id, character.id);
            assert_eq!(owner_hash, "new_owner_hash");
        }
        _ => panic!("Expected UpdateOwnerHash action, got: {:?}", action),
    }
}

/// Tests owned character with different user logged in and matching owner hash.
///
/// Verifies that when a character is owned by a different user than the one
/// logged in, the action is to transfer ownership regardless of hash match.
///
/// Expected: TransferOwnership with to_user_id = Some(logged_in_user_id)
#[test]
fn owned_character_different_user_hash_matches() {
    let session = Session::LoggedIn(42);
    let character = mock_character_model(123456789);
    let ownership = mock_ownership_model(10, character.id, "owner_hash_123");
    let record = CharacterRecord::Owned {
        character: character.clone(),
        ownership,
    };
    let claims = mock_jwt_claims(123456789, "owner_hash_123");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::TransferOwnership {
            to_user_id,
            character: returned_char,
            owner_hash,
        } => {
            assert_eq!(to_user_id, Some(42));
            assert_eq!(returned_char.id, character.id);
            assert_eq!(owner_hash, "owner_hash_123");
        }
        _ => panic!("Expected TransferOwnership action, got: {:?}", action),
    }
}

/// Tests owned character with different user logged in and different owner hash.
///
/// Verifies that when a character is owned by a different user than the one
/// logged in and the hash differs, the action is to transfer ownership.
///
/// Expected: TransferOwnership with to_user_id = Some(logged_in_user_id)
#[test]
fn owned_character_different_user_hash_differs() {
    let session = Session::LoggedIn(42);
    let character = mock_character_model(123456789);
    let ownership = mock_ownership_model(10, character.id, "old_owner_hash");
    let record = CharacterRecord::Owned {
        character: character.clone(),
        ownership,
    };
    let claims = mock_jwt_claims(123456789, "new_owner_hash");

    let action = CallbackService::determine_character_action(session, record, &claims);

    match action {
        CharacterAction::TransferOwnership {
            to_user_id,
            character: returned_char,
            owner_hash,
        } => {
            assert_eq!(to_user_id, Some(42));
            assert_eq!(returned_char.id, character.id);
            assert_eq!(owner_hash, "new_owner_hash");
        }
        _ => panic!("Expected TransferOwnership action, got: {:?}", action),
    }
}

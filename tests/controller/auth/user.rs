use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use bifrost::server::{controller::user::get_user, error::Error};
use sea_orm::{ConnectionTrait, DbBackend, Schema};

use crate::util::setup::{
    test_setup, test_setup_create_character, test_setup_create_corporation,
    test_setup_create_user_with_character, TestSetup,
};

async fn setup() -> Result<TestSetup, Error> {
    let test = test_setup().await;
    let db = &test.state.db;

    let schema = Schema::new(DbBackend::Sqlite);
    let stmts = vec![
        schema.create_table_from_entity(entity::prelude::EveFaction),
        schema.create_table_from_entity(entity::prelude::EveAlliance),
        schema.create_table_from_entity(entity::prelude::EveCorporation),
        schema.create_table_from_entity(entity::prelude::EveCharacter),
        schema.create_table_from_entity(entity::prelude::BifrostUser),
        schema.create_table_from_entity(entity::prelude::BifrostUserCharacter),
    ];

    for stmt in stmts {
        db.execute(&stmt).await?;
    }

    Ok(test)
}

#[tokio::test]
/// Expect 200 success with user information for existing user
async fn returns_success_for_existing_user() -> Result<(), Error> {
    let test = setup().await?;
    let corporation_model = test_setup_create_corporation(&test, 1).await?;
    let character_model = test_setup_create_character(&test, 1, corporation_model).await?;
    let user = test_setup_create_user_with_character(&test, character_model).await?;

    let result = get_user(State(test.state), Path(user.id)).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
/// Expect 404 not found for user that does not exist
async fn returns_not_found_for_user_that_doesnt_exist() -> Result<(), Error> {
    let test = setup().await?;

    let non_existant_user_id = 1;
    let result = get_user(State(test.state), Path(non_existant_user_id)).await;

    assert!(result.is_ok());
    let resp = result.unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
/// Expect 500 internal server error when required database tables dont exist
async fn error_when_required_tables_dont_exist() -> Result<(), Error> {
    let test = test_setup().await;

    let non_existant_user_id = 1;
    let result = get_user(State(test.state), Path(non_existant_user_id)).await;

    assert!(result.is_err());
    let resp = result.err().unwrap().into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

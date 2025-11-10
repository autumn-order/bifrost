use axum::{extract::State, http::StatusCode, response::IntoResponse};
use dioxus_logger::tracing;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tower_sessions::Session;

use crate::{
    model::{
        api::ErrorDto,
        user::{AllianceDto, CharacterDto, CorporationDto},
    },
    server::{
        data::user::user_character::UserCharacterRepository,
        error::Error,
        model::{app::AppState, session::user::SessionUserId},
        service::user::{user_character, UserService},
    },
};

pub static USER_TAG: &str = "user";

// TEMPORARY for alpha 1 tests
/// Get all characters owned by logged in user
#[utoipa::path(
    get,
    path = "/api/user/characters",
    tag = USER_TAG,
    responses(
        (status = 200, description = "Success when retrieving user characters", body = Vec<CharacterDto>),
        (status = 404, description = "User not found", body = ErrorDto),
        (status = 500, description = "Internal server error", body = ErrorDto)
    ),
)]
pub async fn get_user_characters(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, Error> {
    let user_service = UserService::new(&state.db, &state.esi_client);
    let user_character_repository = UserCharacterRepository::new(&state.db);

    let user_id = SessionUserId::get(&session).await?;

    let user_id = if let Some(user_id) = user_id {
        user_id
    } else {
        return Ok((
            StatusCode::NOT_FOUND,
            axum::Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response());
    };

    let user = if let Some(user) = user_service.get_user(user_id).await? {
        user
    } else {
        // Clear session for user not found in database
        session.clear().await;

        tracing::warn!(
            "Failed to find user ID {} in database despite having an active session;
            cleared session for user, they will need to relog to fix",
            user_id
        );

        return Ok((
            StatusCode::NOT_FOUND,
            axum::Json(ErrorDto {
                error: "User not found".to_string(),
            }),
        )
            .into_response());
    };

    let user_characters = user_character_repository
        .get_owned_characters_by_user_id(user.id)
        .await?;

    let corporation_ids = user_characters
        .iter()
        .map(|c| c.corporation_id)
        .collect::<Vec<i32>>();

    // Get related corporations
    let corporations = entity::eve_corporation::Entity::find()
        .filter(entity::eve_corporation::Column::Id.is_in(corporation_ids))
        .all(&state.db)
        .await?;

    let alliance_ids = corporations
        .iter()
        .filter_map(|c| c.alliance_id)
        .collect::<Vec<i32>>();

    let alliances = entity::eve_alliance::Entity::find()
        .filter(entity::eve_alliance::Column::Id.is_in(alliance_ids))
        .all(&state.db)
        .await?;

    let character_dtos: Vec<CharacterDto> = user_characters
        .into_iter()
        .filter_map(|character| {
            let corporation = corporations
                .iter()
                .find(|corp| corp.id == character.corporation_id);

            if let Some(corporation) = corporation {
                let alliance = corporation.alliance_id.and_then(|alliance_id| {
                    alliances.iter().find(|alliance| alliance.id == alliance_id)
                });

                let alliance_dto = if let Some(alliance) = alliance {
                    Some(AllianceDto {
                        id: alliance.alliance_id,
                        name: alliance.name.clone(),
                    })
                } else {
                    None
                };

                Some(CharacterDto {
                    id: character.character_id,
                    name: character.name,
                    corporation: CorporationDto {
                        id: corporation.corporation_id,
                        name: corporation.name.clone(),
                    },
                    alliance: alliance_dto,
                })
            } else {
                None
            }
        })
        .collect();

    Ok((StatusCode::OK, axum::Json(character_dtos)).into_response())
}

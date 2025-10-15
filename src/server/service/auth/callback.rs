use oauth2::TokenResponse;

use crate::server::{error::Error, model::auth::Character};

pub async fn callback_service(
    esi_client: &eve_esi::Client,
    code: String,
) -> Result<Character, Error> {
    let token = esi_client.oauth2().get_token(&code).await?;

    let claims = esi_client
        .oauth2()
        .validate_token(token.access_token().secret().to_string())
        .await?;

    let character_id = claims.character_id()?;

    let character_name = claims.name;
    let character = Character {
        character_id,
        character_name,
    };

    Ok(character)
}

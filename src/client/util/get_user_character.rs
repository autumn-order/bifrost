#[cfg(feature = "web")]
use crate::model::user::CharacterDto;

/// Retrieve user characters from API
#[cfg(feature = "web")]
pub async fn get_user_characters() -> Result<Vec<CharacterDto>, String> {
    use reqwasm::http::Request;

    let response = Request::get("/api/user/characters")
        .credentials(reqwasm::http::RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    match response.status() {
        200 => {
            let chars = response
                .json::<Vec<CharacterDto>>()
                .await
                .map_err(|e| format!("Failed to parse user character data: {}", e))?;
            Ok(chars)
        }
        404 => Ok(Vec::new()),
        _ => {
            use crate::model::api::ErrorDto;

            if let Ok(error_dto) = response.json::<ErrorDto>().await {
                Err(format!(
                    "Request failed with status {}: {}",
                    response.status(),
                    error_dto.error
                ))
            } else {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(format!(
                    "Request failed with status {}: {}",
                    response.status(),
                    error_text
                ))
            }
        }
    }
}

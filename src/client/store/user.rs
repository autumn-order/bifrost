use dioxus::prelude::*;

use crate::model::{api::ErrorDto, user::UserDto};

#[derive(Store)]
pub struct UserState {
    pub user: Option<UserDto>,
    pub fetched: bool,
}

/// Retrieve user from API
#[cfg(feature = "web")]
pub async fn get_user() -> Result<Option<UserDto>, String> {
    use reqwasm::http::Request;

    let response = Request::get("/api/auth/user")
        .credentials(reqwasm::http::RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    match response.status() {
        200 => {
            let user = response
                .json::<UserDto>()
                .await
                .map_err(|e| format!("Failed to parse user data: {}", e))?;
            Ok(Some(user))
        }
        404 => Ok(None),
        _ => {
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

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
/// The response when an error occurs with an API request
pub struct ErrorDto {
    /// The error message
    pub error: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserDto {
    pub user_id: i32,
    pub character_id: i64,
    pub character_name: String,
}

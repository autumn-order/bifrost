use serde::{Deserialize, Serialize};

/// The response when an error occurs with an API request
#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct ErrorDto {
    /// The error message
    pub error: String,
}

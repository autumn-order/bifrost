use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
/// The response when an error occurs with an API request
pub struct ErrorDto {
    /// The error message
    pub error: String,
}

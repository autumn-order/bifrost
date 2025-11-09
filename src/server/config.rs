use crate::server::error::Error;

pub struct Config {
    pub contact_email: String,
    pub esi_client_id: String,
    pub esi_client_secret: String,
    pub esi_callback_url: String,
    pub database_url: String,
    pub valkey_url: String,
    pub user_agent: String,
    pub workers: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let contact_email = std::env::var("CONTACT_EMAIL")
            .map_err(|_| Error::MissingEnvVar("CONTACT_EMAIL".to_string()))?;
        let user_agent = format!(
            "{}/{} ({}; +{})",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            contact_email,
            env!("CARGO_PKG_REPOSITORY")
        );

        Ok(Self {
            contact_email: contact_email,
            esi_client_id: std::env::var("ESI_CLIENT_ID")
                .map_err(|_| Error::MissingEnvVar("ESI_CLIENT_ID".to_string()))?,
            esi_client_secret: std::env::var("ESI_CLIENT_SECRET")
                .map_err(|_| Error::MissingEnvVar("ESI_CLIENT_SECRET".to_string()))?,
            esi_callback_url: std::env::var("ESI_CALLBACK_URL")
                .map_err(|_| Error::MissingEnvVar("ESI_CALLBACK_URL".to_string()))?,
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| Error::MissingEnvVar("DATABASE_URL".to_string()))?,
            valkey_url: std::env::var("VALKEY_URL")
                .map_err(|_| Error::MissingEnvVar("VALKEY_URL".to_string()))?,
            workers: std::env::var("WORKERS")
                .map_err(|_| Error::MissingEnvVar("WORKERS".to_string()))?
                .parse()
                .map_err(|e| Error::InvalidEnvValue {
                    var: "WORKERS".to_string(),
                    reason: format!("must be a valid number: {}", e),
                })?,
            user_agent,
        })
    }
}

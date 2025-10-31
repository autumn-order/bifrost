pub struct Config {
    pub contact_email: String,
    pub esi_client_id: String,
    pub esi_client_secret: String,
    pub esi_callback_url: String,
    pub database_url: String,
    pub valkey_url: String,
    pub user_agent: String,
}

impl Config {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let contact_email = std::env::var("CONTACT_EMAIL")?;
        let user_agent = format!(
            "{}/{} ({}; +{})",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            contact_email,
            env!("CARGO_PKG_REPOSITORY")
        );

        Ok(Self {
            contact_email: contact_email,
            esi_client_id: std::env::var("ESI_CLIENT_ID")?,
            esi_client_secret: std::env::var("ESI_CLIENT_SECRET")?,
            esi_callback_url: std::env::var("ESI_CALLBACK_URL")?,
            database_url: std::env::var("DATABASE_URL")?,
            valkey_url: std::env::var("VALKEY_URL")?,
            user_agent,
        })
    }
}

#![allow(non_snake_case)]

mod client;
mod model;

#[cfg(feature = "server")]
mod server;

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(client::App);
}

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    use dioxus::prelude::*;
    use dioxus_logger::tracing::{info, Level};

    use crate::server::model::app::AppState;

    dotenvy::dotenv().ok();

    let contact_email = std::env::var("CONTACT_EMAIL").expect("CONTACT_EMAIL is not set in .env");

    let user_agent = format!(
        "{}/{} ({}; +{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        contact_email,
        env!("CARGO_PKG_REPOSITORY")
    );

    let esi_client = build_esi_client(&user_agent).unwrap();
    let session = connect_to_session();
    let db = connect_to_database().await.unwrap();

    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("Starting server");

    let state = AppState {
        db,
        esi_client: esi_client,
    };

    let router = server::router::routes()
        .serve_dioxus_application(ServeConfigBuilder::default(), client::App)
        .with_state(state)
        .layer(session);

    let router = router.into_make_service();
    let address = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

#[cfg(feature = "server")]
pub fn build_esi_client(user_agent: &str) -> Result<eve_esi::Client, eve_esi::Error> {
    let esi_client_id = std::env::var("ESI_CLIENT_ID").expect("ESI_CLIENT_ID is not set in .env");
    let esi_client_secret =
        std::env::var("ESI_CLIENT_SECRET").expect("ESI_CLIENT_SECRET is not set in .env");
    let esi_callback_url =
        std::env::var("ESI_CALLBACK_URL").expect("ESI_CALLBACK_URL is not set in .env");

    eve_esi::Client::builder()
        .user_agent(&user_agent)
        .client_id(&esi_client_id)
        .client_secret(&esi_client_secret)
        .callback_url(&esi_callback_url)
        .build()
}

#[cfg(feature = "server")]
pub async fn connect_to_database() -> Result<sea_orm::DatabaseConnection, sea_orm::DbErr> {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env");

    let mut opt = ConnectOptions::new(database_url);
    opt.sqlx_logging(false);

    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to database");

    Migrator::up(&db, None)
        .await
        .expect("Failed to run database migrations.");

    Ok(db)
}

#[cfg(feature = "server")]
pub fn connect_to_session() -> tower_sessions::SessionManagerLayer<tower_sessions::MemoryStore> {
    use time::Duration;
    use tower_sessions::{cookie::SameSite, Expiry, MemoryStore, SessionManagerLayer};

    // Set secure based on build mode: in development (debug) use false, otherwise true.
    let development_mode = cfg!(debug_assertions);
    let secure_cookies = !development_mode;

    let session_store = MemoryStore::default();
    SessionManagerLayer::new(session_store)
        .with_secure(secure_cookies)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(120)))
}

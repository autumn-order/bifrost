#![allow(non_snake_case)]

mod client;
mod model;

#[cfg(feature = "server")]
use tower_sessions::SessionManagerLayer;
#[cfg(feature = "server")]
use tower_sessions_redis_store::{fred::prelude::Pool, RedisStore};

#[cfg(feature = "server")]
use bifrost::server;
#[cfg(feature = "server")]
use server::error::Error;

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(client::App);

    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use dioxus_logger::tracing::info;

        use crate::server::{config::Config, model::app::AppState};

        dotenvy::dotenv().ok();

        let config = Config::from_env().unwrap();

        let user_agent = format!(
            "{}/{} ({}; +{})",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            config.contact_email,
            env!("CARGO_PKG_REPOSITORY")
        );

        let esi_client = build_esi_client(
            &user_agent,
            &config.esi_client_id,
            &config.esi_client_secret,
            &config.esi_callback_url,
        )
        .unwrap();
        let session = connect_to_session(&config.valkey_url).await.unwrap();
        let db = connect_to_database(&config.database_url).await.unwrap();

        info!("Starting server");

        let state = AppState {
            db,
            esi_client: esi_client,
        };

        let mut router = dioxus::server::router(client::App);

        let server = server::router::routes().with_state(state).layer(session);
        router = router.merge(server);

        Ok(router)
    })
}

#[cfg(feature = "server")]
pub fn build_esi_client(
    user_agent: &str,
    esi_client_id: &str,
    esi_client_secret: &str,
    esi_callback_url: &str,
) -> Result<eve_esi::Client, Error> {
    let esi_client = eve_esi::Client::builder()
        .user_agent(&user_agent)
        .client_id(&esi_client_id)
        .client_secret(&esi_client_secret)
        .callback_url(&esi_callback_url)
        .build()?;

    Ok(esi_client)
}

#[cfg(feature = "server")]
pub async fn connect_to_database(database_url: &str) -> Result<sea_orm::DatabaseConnection, Error> {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

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
pub async fn connect_to_session(
    valkey_url: &str,
) -> Result<SessionManagerLayer<RedisStore<Pool>>, Error> {
    use time::Duration;
    use tower_sessions::{cookie::SameSite, Expiry, SessionManagerLayer};
    use tower_sessions_redis_store::{fred::prelude::*, RedisStore};

    let config = Config::from_url(&valkey_url)?;
    let pool = Pool::new(config, None, None, None, 6)?;

    pool.connect();
    pool.wait_for_connect().await?;

    let session_store = RedisStore::new(pool);

    // Set secure based on build mode: in development (debug) use false, otherwise true.
    let development_mode = cfg!(debug_assertions);
    let secure_cookies = !development_mode;

    let session = SessionManagerLayer::new(session_store)
        .with_secure(secure_cookies)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(120)));

    Ok(session)
}

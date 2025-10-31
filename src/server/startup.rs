use crate::server::error::Error;
use tower_sessions::SessionManagerLayer;
use tower_sessions_redis_store::{fred::prelude::Pool, RedisStore};

/// Build and configure the ESI client with the provided credentials
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

/// Connect to the database and run migrations
pub async fn connect_to_database(
    database_url: &str,
) -> Result<sea_orm::DatabaseConnection, Error> {
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

/// Connect to Valkey/Redis and configure session management
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

use fred::prelude::*;
use sea_orm::DatabaseConnection;
use tower_sessions::SessionManagerLayer;
use tower_sessions_redis_store::RedisStore;

use crate::server::{
    config::Config,
    error::Error,
    worker::{handler::WorkerJobHandler, Worker},
};

/// Build and configure the ESI client with the provided credentials
pub fn build_esi_client(config: &Config) -> Result<eve_esi::Client, Error> {
    let esi_client = eve_esi::Client::builder()
        .user_agent(&config.user_agent)
        .client_id(&config.esi_client_id)
        .client_secret(&config.esi_client_secret)
        .callback_url(&config.esi_callback_url)
        .build()?;

    Ok(esi_client)
}

/// Connect to the database and run migrations
pub async fn connect_to_database(config: &Config) -> Result<sea_orm::DatabaseConnection, Error> {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

    let mut opt = ConnectOptions::new(&config.database_url);
    opt.sqlx_logging(false);

    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to database");

    Migrator::up(&db, None)
        .await
        .expect("Failed to run database migrations.");

    Ok(db)
}

pub async fn connect_to_redis(config: &Config) -> Result<Pool, Error> {
    let config = fred::prelude::Config::from_url(&config.valkey_url)?;
    let pool = Pool::new(config, None, None, None, 6)?;

    pool.connect();
    pool.wait_for_connect().await?;

    Ok(pool)
}

/// Connect to Valkey/Redis and configure session management
pub async fn connect_to_session(
    redis_pool: Pool,
) -> Result<SessionManagerLayer<RedisStore<Pool>>, Error> {
    use time::Duration;
    use tower_sessions::{cookie::SameSite, Expiry, SessionManagerLayer};

    let session_store = RedisStore::new(redis_pool);

    // Set secure based on build mode: in development (debug) use false, otherwise true.
    let development_mode = cfg!(debug_assertions);
    let secure_cookies = !development_mode;

    let session = SessionManagerLayer::new(session_store)
        .with_secure(secure_cookies)
        .with_same_site(SameSite::Lax)
        .with_http_only(true)
        .with_expiry(Expiry::OnInactivity(Duration::days(7)));

    Ok(session)
}

pub async fn start_workers(
    config: &Config,
    db: DatabaseConnection,
    redis_pool: Pool,
    esi_client: eve_esi::Client,
) -> Result<Worker, Error> {
    let handler = WorkerJobHandler::new(db, esi_client);
    let worker = Worker::new(config.workers, redis_pool.clone(), handler);

    worker.pool.start().await?;

    Ok(worker)
}

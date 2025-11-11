use apalis_redis::RedisStorage;
use sea_orm::DatabaseConnection;
use tower_sessions::SessionManagerLayer;
use tower_sessions_redis_store::RedisStore;

use crate::server::{config::Config, error::Error, model::worker::WorkerJob, worker::handle_job};

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

/// Connect to Valkey/Redis and configure session management
pub async fn connect_to_session(
    config: &Config,
) -> Result<SessionManagerLayer<RedisStore<tower_sessions_redis_store::fred::prelude::Pool>>, Error>
{
    use time::Duration;
    use tower_sessions::{cookie::SameSite, Expiry, SessionManagerLayer};
    use tower_sessions_redis_store::fred::prelude::*;

    let config = Config::from_url(&config.valkey_url)?;
    let pool = tower_sessions_redis_store::fred::prelude::Pool::new(config, None, None, None, 6)?;

    pool.connect();
    pool.wait_for_connect().await?;

    let session_store = RedisStore::new(pool);

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

/// Connect to Redis for job tracking
pub async fn connect_to_job_tracker(config: &Config) -> Result<fred::prelude::Pool, Error> {
    use fred::prelude::*;

    let redis_config = Config::from_url(&config.valkey_url)?;
    let pool = Pool::new(redis_config, None, None, None, 6)?;

    pool.connect();
    pool.wait_for_connect().await?;

    Ok(pool)
}

pub async fn start_workers(
    config: &Config,
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
    redis_pool: fred::prelude::Pool,
) -> Result<RedisStorage<WorkerJob>, Error> {
    use apalis::prelude::*;

    let conn = apalis_redis::connect(config.valkey_url.to_string()).await?;
    let storage = RedisStorage::new(conn);
    let workers = config.workers;

    let storage_clone = storage.clone();

    let _ = tokio::spawn(async move {
        WorkerBuilder::new("bifrost-worker")
            .concurrency(workers)
            .data(db)
            .data(esi_client)
            .data(redis_pool)
            .backend(storage_clone)
            .build_fn(handle_job)
            .run()
            .await;
    });

    Ok(storage)
}

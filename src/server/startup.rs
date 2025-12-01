use dioxus_logger::tracing;
use fred::prelude::*;
use sea_orm::DatabaseConnection;
use tower_sessions::SessionManagerLayer;
use tower_sessions_redis_store::RedisStore;

use crate::server::{
    config::Config,
    error::Error,
    scheduler::Scheduler,
    worker::{handler::WorkerJobHandler, Worker, WorkerQueue},
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

    let db = Database::connect(opt).await?;

    Migrator::up(&db, None).await?;

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

/// Initialize and start the scheduler in a background task.
///
/// Creates a new scheduler instance and spawns it in a detached Tokio task to run independently
/// in the background. The scheduler will register all EVE Online data refresh jobs (factions,
/// alliances, corporations, characters, and affiliations) and begin executing them according to
/// their configured cron schedules.
///
/// The scheduler runs in a fire-and-forget manner - errors are logged but do not propagate back
/// to the caller. This ensures that scheduler failures don't bring down the main application.
///
/// # Arguments
/// - `db` - Database connection for querying entities that need updates
/// - `queue` - Worker queue for dispatching asynchronous refresh tasks
///
/// # Returns
/// - `Ok(())` - Scheduler successfully created and background task spawned
/// - `Err(Error)` - Failed to initialize the scheduler (occurs before spawning)
pub async fn start_scheduler(db: DatabaseConnection, queue: WorkerQueue) -> Result<(), Error> {
    let scheduler = Scheduler::new(db, queue).await?;

    tokio::spawn(async move {
        if let Err(e) = scheduler.start().await {
            tracing::error!("Scheduler error: {:?}", e);
        }

        tracing::info!("Job scheduler started");
    });

    Ok(())
}

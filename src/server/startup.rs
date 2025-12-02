//! Server startup and initialization functions.
//!
//! This module provides functions for initializing and configuring all server components
//! during application startup. This includes connecting to databases and Redis, building
//! the ESI client with OAuth credentials, configuring session management, starting background
//! workers, and initializing the job scheduler. Each function handles a specific aspect of
//! server initialization with proper error handling.

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

/// Builds and configures the ESI client with OAuth credentials from configuration.
///
/// Creates an EVE Online ESI API client configured with the application's OAuth credentials,
/// user agent string, and callback URL. The user agent includes contact information required
/// by ESI's guidelines. The client is used for all ESI API interactions including OAuth
/// authentication and data fetching.
///
/// # Arguments
/// - `config` - Application configuration containing ESI credentials and user agent
///
/// # Returns
/// - `Ok(eve_esi::Client)` - Configured ESI client ready for API requests
/// - `Err(Error)` - Failed to build the ESI client (invalid configuration)
///
/// # Example
/// ```ignore
/// let config = Config::from_env()?;
/// let esi_client = build_esi_client(&config)?;
/// // Client is ready for OAuth flows and ESI requests
/// ```
pub fn build_esi_client(config: &Config) -> Result<eve_esi::Client, Error> {
    let esi_client = eve_esi::Client::builder()
        .user_agent(&config.user_agent)
        .client_id(&config.esi_client_id)
        .client_secret(&config.esi_client_secret)
        .callback_url(&config.esi_callback_url)
        .build()?;

    Ok(esi_client)
}

/// Connects to the PostgreSQL database and runs pending migrations.
///
/// Establishes a connection pool to the PostgreSQL database using the connection string from
/// configuration, then automatically runs all pending SeaORM migrations to ensure the database
/// schema is up-to-date. This function must complete successfully before the application can
/// access the database.
///
/// # Arguments
/// - `config` - Application configuration containing the database URL
///
/// # Returns
/// - `Ok(DatabaseConnection)` - Connected database with migrations applied
/// - `Err(Error)` - Failed to connect to database or run migrations
///
/// # Example
/// ```ignore
/// let config = Config::from_env()?;
/// let db = connect_to_database(&config).await?;
/// // Database is ready for queries
/// ```
pub async fn connect_to_database(config: &Config) -> Result<sea_orm::DatabaseConnection, Error> {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

    let mut opt = ConnectOptions::new(&config.database_url);
    opt.sqlx_logging(false);

    let db = Database::connect(opt).await?;

    Migrator::up(&db, None).await?;

    Ok(db)
}

/// Connects to Redis/Valkey and creates a connection pool.
///
/// Establishes a connection pool to the Redis/Valkey server using the connection string from
/// configuration. The pool is configured with 6 connections and is used for both session
/// storage and the worker queue backend. This function waits for the connection to be
/// established before returning.
///
/// # Arguments
/// - `config` - Application configuration containing the Valkey/Redis URL
///
/// # Returns
/// - `Ok(Pool)` - Connected Redis pool ready for use
/// - `Err(Error)` - Failed to parse URL, create pool, or establish connection
///
/// # Example
/// ```ignore
/// let config = Config::from_env()?;
/// let redis_pool = connect_to_redis(&config).await?;
/// // Pool is ready for session storage and worker queue
/// ```
pub async fn connect_to_redis(config: &Config) -> Result<Pool, Error> {
    let config = fred::prelude::Config::from_url(&config.valkey_url)?;
    let pool = Pool::new(config, None, None, None, 6)?;

    pool.connect();
    pool.wait_for_connect().await?;

    Ok(pool)
}

/// Configures session management with Redis/Valkey backend.
///
/// Creates a session manager layer configured with Redis storage, cookie settings, and
/// expiration policies. Session cookies are configured based on build mode: secure cookies
/// are enabled in production (release builds) and disabled in development (debug builds) to
/// allow testing over HTTP. Sessions expire after 7 days of inactivity.
///
/// # Cookie Configuration
/// - **Secure**: Enabled in release builds, disabled in debug builds
/// - **SameSite**: Lax (allows top-level navigation)
/// - **HttpOnly**: Enabled (prevents JavaScript access)
/// - **Expiry**: 7 days of inactivity
///
/// # Arguments
/// - `redis_pool` - Connected Redis pool for session storage
///
/// # Returns
/// - `Ok(SessionManagerLayer)` - Configured session middleware ready for Axum
/// - `Err(Error)` - Failed to create session store (unlikely with valid pool)
///
/// # Example
/// ```ignore
/// let redis_pool = connect_to_redis(&config).await?;
/// let session_layer = connect_to_session(redis_pool).await?;
/// // Session layer can be added to Axum router
/// ```
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

/// Initializes and starts the background worker system.
///
/// Creates a worker pool with the configured number of worker threads, initializes the job
/// handler with database and ESI client, and starts the worker pool to begin processing jobs
/// from the Redis-backed queue. Workers handle background tasks like ESI data refresh for
/// alliances, corporations, characters, and affiliations.
///
/// # Arguments
/// - `config` - Application configuration containing worker pool size
/// - `db` - Database connection for workers to persist data
/// - `redis_pool` - Redis pool for the worker queue backend
/// - `esi_client` - ESI client for workers to fetch data from EVE Online
///
/// # Returns
/// - `Ok(Worker)` - Started worker system ready to process jobs
/// - `Err(Error)` - Failed to create or start worker pool
///
/// # Example
/// ```ignore
/// let worker = start_workers(&config, db, redis_pool, esi_client).await?;
/// // Workers are now processing jobs from the queue
/// ```
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
/// to the caller.
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

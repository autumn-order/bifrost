//! Bifrost - EVE Online user and character management application.
//!
//! This application provides a full-stack solution for managing EVE Online users and their
//! characters, with OAuth authentication via EVE SSO, automated ESI data refresh, and a
//! web-based UI. The application can run in two modes: client-only (for frontend development)
//! or full-stack server mode with backend API, database, worker queue, and job scheduler.

mod model;

use bifrost::server::{self, error::AppError};

/// Application entry point
///
/// # Server
/// - **Configuration**: Loads environment variables and validates required settings
/// - **Database**: Connects to PostgreSQL and runs migrations
/// - **Redis/Valkey**: Establishes connection pool for sessions and worker queue
/// - **Session Management**: Configures secure session cookies with Redis backend
/// - **ESI Client**: Builds OAuth-enabled EVE Online API client
/// - **Worker System**: Starts background worker pool for ESI data refresh jobs
/// - **Job Scheduler**: Initializes cron-based scheduler for automated data updates
#[tokio::main]
async fn main() -> Result<(), AppError> {
    use crate::server::{config::Config, model::app::AppState, startup};

    dotenvy::dotenv().ok();
    let config = Config::from_env()?;

    let db = startup::connect_to_database(&config).await?;
    let redis_pool = startup::connect_to_redis(&config).await?;
    let session = startup::connect_to_session(redis_pool.clone()).await?;
    let esi_client = startup::build_esi_client(&config)?;

    let esi_provider = server::service::eve::esi::EsiProvider::new(esi_client);

    let worker =
        startup::start_workers(&config, db.clone(), redis_pool, esi_provider.clone()).await?;
    startup::start_scheduler(db.clone(), worker.queue.clone()).await?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

    axum::serve(
        listener,
        server::router::routes()
            .with_state(AppState {
                db,
                esi_provider,
                worker,
            })
            .layer(session),
    )
    .await?;

    Ok(())
}

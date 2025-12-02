//! Bifrost - EVE Online user and character management application.
//!
//! This application provides a full-stack solution for managing EVE Online users and their
//! characters, with OAuth authentication via EVE SSO, automated ESI data refresh, and a
//! web-based UI. The application can run in two modes: client-only (for frontend development)
//! or full-stack server mode with backend API, database, worker queue, and job scheduler.

#![allow(non_snake_case)]

mod client;
mod model;

#[cfg(feature = "server")]
use bifrost::server;

/// Application entry point
///
/// # Client
/// Dioxus-based frontend providing webui for user interaction with the auth
///
/// # Full-Stack Server (`--features server`)
/// When the `server` feature is enabled, launches the backend with with:
/// - **Configuration**: Loads environment variables and validates required settings
/// - **Database**: Connects to PostgreSQL and runs migrations
/// - **Redis/Valkey**: Establishes connection pool for sessions and worker queue
/// - **Session Management**: Configures secure session cookies with Redis backend
/// - **ESI Client**: Builds OAuth-enabled EVE Online API client
/// - **Worker System**: Starts background worker pool for ESI data refresh jobs
/// - **Job Scheduler**: Initializes cron-based scheduler for automated data updates
/// - **HTTP Router**: Configures API routes and Dioxus SSR with middleware
///
/// # Startup Sequence (Server)
/// 1. Load `.env` file if present (for local development)
/// 2. Load and validate configuration from environment variables
/// 3. Connect to PostgreSQL database and run pending migrations
/// 4. Connect to Redis/Valkey for sessions and worker queue
/// 5. Configure session management with secure cookies
/// 6. Build ESI client with OAuth credentials
/// 7. Start background worker pool to process jobs
/// 8. Start job scheduler to enqueue periodic refresh jobs
/// 9. Build combined router (Dioxus SSR + API routes + session middleware)
/// 10. Start HTTP server
///
/// # Environment Variables (Server)
/// See `server::config::Config::from_env()` for required environment variables including
/// database URLs, ESI credentials, contact email, and worker pool size.
///
/// # Panics
/// Panics if server initialization fails (missing environment variables, connection failures,
/// etc.) as the application cannot function without required infrastructure.
fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(client::App);

    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use dioxus_logger::tracing;

        use crate::server::{config::Config, model::app::AppState, startup};

        dotenvy::dotenv().ok();
        let config = Config::from_env()?;

        let db = startup::connect_to_database(&config).await?;
        let redis_pool = startup::connect_to_redis(&config).await?;
        let session = startup::connect_to_session(redis_pool.clone()).await?;
        let esi_client = startup::build_esi_client(&config)?;

        let worker =
            startup::start_workers(&config, db.clone(), redis_pool, esi_client.clone()).await?;
        startup::start_scheduler(db.clone(), worker.queue.clone()).await?;

        tracing::info!("Starting server");

        let mut router = dioxus::server::router(client::App);
        let server_routes = server::router::routes()
            .with_state(AppState {
                db,
                esi_client,
                worker,
            })
            .layer(session);
        router = router.merge(server_routes);

        Ok(router)
    })
}

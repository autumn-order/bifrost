#![allow(non_snake_case)]

mod client;
mod model;

#[cfg(feature = "server")]
use bifrost::server;

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(client::App);

    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use bifrost::server::scheduler::cron::start_scheduler;
        use dioxus_logger::tracing;

        use crate::server::{config::Config, model::app::AppState, startup};

        dotenvy::dotenv().ok();
        let config = match Config::from_env() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Configuration error: {}", e);
                std::process::exit(1);
            }
        };

        let esi_client = startup::build_esi_client(&config).unwrap();
        let session = startup::connect_to_session(&config).await.unwrap();
        let db = startup::connect_to_database(&config).await.unwrap();
        let redis_pool = startup::connect_to_job_tracker(&config).await.unwrap();
        let mut worker_storage =
            startup::start_workers(&config, db.clone(), esi_client.clone(), redis_pool.clone())
                .await
                .unwrap();
        let _ = start_scheduler(&db, &redis_pool, &esi_client, &mut worker_storage)
            .await
            .unwrap();

        tracing::info!("Starting server");

        let mut router = dioxus::server::router(client::App);
        let server_routes = server::router::routes()
            .with_state(AppState { db, esi_client })
            .layer(session);
        router = router.merge(server_routes);

        Ok(router)
    })
}

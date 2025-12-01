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

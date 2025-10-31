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
        use dioxus_logger::tracing::info;

        use crate::server::{config::Config, model::app::AppState, startup};

        dotenvy::dotenv().ok();
        let config = Config::from_env().unwrap();
        let user_agent = format!(
            "{}/{} ({}; +{})",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            config.contact_email,
            env!("CARGO_PKG_REPOSITORY")
        );

        let esi_client = startup::build_esi_client(
            &user_agent,
            &config.esi_client_id,
            &config.esi_client_secret,
            &config.esi_callback_url,
        )
        .unwrap();
        let session = startup::connect_to_session(&config.valkey_url)
            .await
            .unwrap();
        let db = startup::connect_to_database(&config.database_url)
            .await
            .unwrap();

        info!("Starting server");

        let mut router = dioxus::server::router(client::App);
        let server_routes = server::router::routes()
            .with_state(AppState {
                db,
                esi_client: esi_client,
            })
            .layer(session);
        router = router.merge(server_routes);

        Ok(router)
    })
}

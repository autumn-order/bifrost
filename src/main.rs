#![allow(non_snake_case)]

mod client;
mod model;

#[cfg(feature = "server")]
mod server;

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(client::App);
}

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use dioxus::prelude::*;
    use dioxus_logger::tracing::{info, Level};

    use crate::server::router::AppState;

    dotenvy::dotenv().ok();

    let contact_email = std::env::var("CONTACT_EMAIL").expect("CONTACT_EMAIL is not set in .env");
    let esi_client_id = std::env::var("ESI_CLIENT_ID").expect("ESI_CLIENT_ID is not set in .env");
    let esi_client_secret =
        std::env::var("ESI_CLIENT_SECRET").expect("ESI_CLIENT_SECRET is not set in .env");
    let esi_callback_url =
        std::env::var("ESI_CALLBACK_URL").expect("ESI_CALLBACK_URL is not set in .env");

    let user_agent = format!(
        "{}/{} ({}; +{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        contact_email,
        env!("CARGO_PKG_REPOSITORY")
    );

    let esi_client = eve_esi::Client::builder()
        .user_agent(&user_agent)
        .client_id(&esi_client_id)
        .client_secret(&esi_client_secret)
        .callback_url(&esi_callback_url)
        .build()
        .expect("Failed to build ESI client");

    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("Starting server");

    let state = AppState {
        esi_client: esi_client,
    };

    let router = Router::new()
        .serve_dioxus_application(ServeConfigBuilder::default(), client::App)
        .with_state(state);

    let router = router.into_make_service();
    let address = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

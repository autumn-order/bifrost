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

    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("Starting server");

    let router = Router::new().serve_dioxus_application(ServeConfigBuilder::default(), client::App);

    let router = router.into_make_service();
    let address = dioxus_cli_config::fullstack_address_or_localhost();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

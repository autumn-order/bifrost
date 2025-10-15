use axum::routing::get;
use axum::Router;

use crate::server::{controller, model::app::AppState};

pub fn routes() -> Router<AppState> {
    let auth_routes = Router::new().route("/auth/login", get(controller::auth::login::login));

    let routes = Router::new().merge(auth_routes);

    routes
}

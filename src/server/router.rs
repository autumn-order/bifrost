use axum::routing::get;
use axum::Router;

use crate::server::{controller, model::app::AppState};

pub fn routes() -> Router<AppState> {
    let auth_routes = Router::new()
        .route("/auth/login", get(controller::auth::login))
        .route("/auth/callback", get(controller::auth::callback))
        .route("/auth/logout", get(controller::auth::logout));

    let user_routes = Router::new().route("/user/{id}", get(controller::user::get_user));

    let routes = Router::new().merge(auth_routes).merge(user_routes);

    routes
}

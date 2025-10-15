use axum::Router;

use crate::server::model::app::AppState;

pub fn routes() -> Router<AppState> {
    let routes = Router::new();

    routes
}

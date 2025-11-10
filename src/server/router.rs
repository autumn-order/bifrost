use axum::Router;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::server::{controller, model::app::AppState};

pub fn routes() -> Router<AppState> {
    #[derive(OpenApi)]
    #[openapi(info(title = "Bifrost", description = "Bifrost API"), tags(
        (name = controller::auth::AUTH_TAG, description = "Authentication API routes"),
    ))]
    struct ApiDoc;

    let (routes, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(controller::auth::login))
        .routes(routes!(controller::auth::callback))
        .routes(routes!(controller::auth::logout))
        .routes(routes!(controller::auth::get_user))
        .split_for_parts();

    let routes = routes.merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", api));

    routes
}

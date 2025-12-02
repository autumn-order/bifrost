//! HTTP routing and OpenAPI documentation configuration.
//!
//! This module defines the application's HTTP routes and generates OpenAPI documentation
//! using utoipa. All API endpoints are registered here with their OpenAPI specifications,
//! and Swagger UI is configured to provide interactive API documentation at `/api/docs`.

use axum::Router;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::server::{controller, model::app::AppState};

/// Builds the application's HTTP router with all API endpoints and Swagger UI documentation.
///
/// Constructs an Axum router with all authentication and user management endpoints registered.
/// Each endpoint is annotated with OpenAPI specifications via utoipa, which are collected into
/// a unified OpenAPI document. The router includes Swagger UI at `/api/docs` for interactive
/// API exploration and testing.
///
/// # Registered Endpoints
/// - `POST /api/auth/login` - Initiate EVE Online SSO authentication
/// - `GET /api/auth/callback` - OAuth callback handler
/// - `GET /api/auth/logout` - Logout current user
/// - `GET /api/auth/user` - Get current user information
/// - `GET /api/user/characters` - Get characters owned by current user
///
/// # OpenAPI Documentation
/// The OpenAPI specification is available at `/api/docs/openapi.json` and includes:
/// - Endpoint paths and HTTP methods
/// - Request/response schemas
/// - Authentication requirements
/// - Error responses
///
/// # Swagger UI
/// Interactive API documentation is served at `/api/docs`, allowing developers to:
/// - Browse available endpoints
/// - View request/response schemas
/// - Test endpoints directly from the browser
/// - Download the OpenAPI specification
///
/// # Returns
/// An Axum `Router<AppState>` configured with all routes and middleware, ready to be
/// merged into the main application router.
///
/// # Example
/// ```ignore
/// let app_state = AppState { db, esi_client, worker };
/// let router = routes().with_state(app_state);
/// // Router is now ready to serve HTTP requests
/// ```
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
        .routes(routes!(controller::user::get_user_characters))
        .split_for_parts();

    let routes = routes.merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", api));

    routes
}

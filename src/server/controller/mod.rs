//! HTTP controller endpoints for the Bifrost web API.
//!
//! This module contains Axum handlers for authentication, user management, and related
//! functionality. Controllers handle HTTP requests, validate inputs, interact with services,
//! and return appropriate HTTP responses. They integrate with tower-sessions for session
//! management and use utoipa for OpenAPI documentation.

pub mod auth;
pub mod user;
pub mod util;

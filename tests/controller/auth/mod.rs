//! Tests for authentication controller endpoints.
//!
//! This module contains integration tests for authentication-related HTTP endpoints,
//! including EVE Online SSO login flow, OAuth callback handling, logout functionality,
//! and authenticated user information retrieval.

mod callback;
mod login;
mod logout;
mod user;

use super::*;

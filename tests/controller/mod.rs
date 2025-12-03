//! Tests for HTTP controller endpoints.
//!
//! This module contains integration tests for the application's HTTP controllers,
//! verifying request handling, response formatting, authentication flows, and error
//! handling for all API endpoints.

mod auth;
mod user;

use bifrost_test_utils::prelude::*;

use crate::util::TestContextExt;

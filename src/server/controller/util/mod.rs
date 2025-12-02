//! Utility functions for controller request handling.
//!
//! This module provides reusable helper functions used across controllers, including
//! CSRF token validation for authentication flows and user session retrieval for
//! protected endpoints.

pub mod csrf;
pub mod get_user;

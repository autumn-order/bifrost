//! Session data models and utilities.
//!
//! This module provides type-safe wrappers for session data storage and retrieval using
//! tower-sessions. Each submodule defines a specific piece of session state (user ID,
//! CSRF tokens, flags) with methods for inserting, retrieving, and removing data from
//! the session store (Redis-backed).

pub mod auth;
pub mod change_main;
pub mod user;

//! Test fixture modules for database and HTTP mock creation.
//!
//! This module contains fixture utilities for creating test data and mock HTTP endpoints
//! during test execution (Phase 2 of the test architecture). Each submodule provides
//! specialized fixtures for different aspects of the system:
//!
//! - `auth` - JWT tokens and OAuth2 authentication endpoints
//! - `eve` - EVE Online entity data (factions, alliances, corporations, characters)
//! - `user` - Bifrost user and character ownership records

pub mod auth;
pub mod eve;
pub mod user;

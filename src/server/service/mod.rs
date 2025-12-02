//! Service layer for business logic and orchestration.
//!
//! This module contains the service layer that implements business logic, coordinates
//! between repositories and external APIs, and handles complex multi-step operations.
//! Services include authentication, EVE Online data management, orchestration for
//! dependency resolution, retry logic, and user management.

pub mod auth;
pub mod eve;
pub mod orchestrator;
pub mod retry;
pub mod user;

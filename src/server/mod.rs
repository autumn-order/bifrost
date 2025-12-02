//! Server application core modules.
//!
//! This module contains all server-side functionality for the Bifrost application, including
//! HTTP routing, authentication, database operations, background workers, job scheduling, and
//! EVE Online ESI integration. It provides the complete backend infrastructure for managing
//! user accounts, EVE character data, and automated data refresh operations.

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod config;
pub mod controller;
pub mod data;
pub mod error;
pub mod model;
pub mod router;
pub mod scheduler;
pub mod service;
pub mod startup;
pub mod util;
pub mod worker;

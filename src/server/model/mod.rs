//! Server application models and type definitions.
//!
//! This module contains data models for the server application, including application state,
//! database model type aliases, session data structures, and worker job definitions. These
//! models bridge the gap between database entities, HTTP handlers, and background workers.

pub mod app;
pub mod db;
pub mod session;
pub mod worker;

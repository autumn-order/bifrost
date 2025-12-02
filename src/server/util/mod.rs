//! Utility functions and helpers for server operations.
//!
//! This module provides reusable utility functions for common server tasks, including
//! EVE Online-specific operations (character ID validation, ESI limits) and time/date
//! calculations (cache expiry determination). These utilities are used across services,
//! workers, and schedulers.

pub mod eve;
pub mod time;

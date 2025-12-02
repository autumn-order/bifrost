//! Authentication service layer.
//!
//! This module contains business logic services for handling EVE Online SSO authentication.
//! Services manage the OAuth2 flow including login URL generation and callback processing
//! with character ownership management.

pub mod callback;
pub mod login;

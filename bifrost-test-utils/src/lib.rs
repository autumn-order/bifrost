//! Test utilities for bifrost integration and unit tests
//!
//! This crate provides a declarative TestBuilder API for creating test environments
//! with in-memory SQLite databases, mock ESI servers, and session management.
//!
//! # Primary APIs
//!
//! - [`TestBuilder`] - Declarative builder for test setup (primary entry point)
//! - [`factory`] - Factory functions for creating mock EVE ESI model objects
//!
//! # Examples
//!
//! ## Basic test setup
//!
//! ```ignore
//! use bifrost_test_utils::prelude::*;
//!
//! #[tokio::test]
//! async fn my_test() -> Result<(), TestError> {
//!     let test = TestBuilder::new().build().await?;
//!
//!     // Use test.db, test.esi_client, test.session
//!     Ok(())
//! }
//! ```
//!
//! ## With database tables
//!
//! ```ignore
//! let test = TestBuilder::new()
//!     .with_table(entity::prelude::EveFaction)
//!     .with_table(entity::prelude::EveAlliance)
//!     .build()
//!     .await?;
//! ```
//!
//! ## With user tables
//!
//! ```ignore
//! let test = TestBuilder::new()
//!     .with_user_tables()
//!     .build()
//!     .await?;
//! ```
//!
//! ## With mock endpoints and fixtures
//!
//! ```ignore
//! let faction_id = 1;
//! let mock_faction = factory::mock_faction(faction_id);
//!
//! let test = TestBuilder::new()
//!     .with_table(entity::prelude::EveFaction)
//!     .with_mock_faction(faction_id)
//!     .with_faction_endpoint(vec![mock_faction], 1)
//!     .build()
//!     .await?;
//!
//! // Verify mocks were called
//! test.assert_mocks();
//! ```

pub mod builder;
pub mod constant;
pub mod error;
pub mod model;

// Internal modules (not exposed in public API)
mod fixtures;
mod setup;

// Re-export primary API types
pub use builder::TestBuilder;
pub use error::TestError;

// Re-export TestSetup for use in custom test utilities
// (e.g., creating AppState in integration tests)
pub use setup::TestSetup;

/// Prelude module containing commonly used imports for tests
///
/// # Usage
///
/// ```ignore
/// use bifrost_test_utils::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        builder::TestBuilder, error::TestError, fixtures::eve::factory, setup::TestSetup,
    };
}

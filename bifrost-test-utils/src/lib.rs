#![warn(missing_docs)]

//! Test utilities for bifrost integration and unit tests
//!
//! This crate provides a declarative TestBuilder API for creating test environments
//! with in-memory SQLite databases, mock ESI servers, and session management.
//!
//! # Architecture
//!
//! Bifrost's test utilities use a **two-phase architecture**:
//!
//! ## Phase 1: TestBuilder (Declarative Setup)
//!
//! Configure your test environment before execution. All operations are queued
//! and executed during `.build()`:
//!
//! ```ignore
//! use bifrost_test_utils::prelude::*;
//!
//! let test = TestBuilder::new()
//!     .with_user_tables()              // Queue table creation
//!     .with_mock_faction(1)            // Queue faction insertion
//!     .with_faction_endpoint(...)      // Queue HTTP mock
//!     .build()                         // Execute all queued operations
//!     .await?;
//! ```
//!
//! ## Phase 2: TestContext Fixtures (Imperative Operations)
//!
//! Perform operations during test execution using the returned [`TestContext`]:
//!
//! ```ignore
//! // Create data objects (no side effects)
//! let faction = factory::mock_faction(1);
//!
//! // Insert into database (side effect)
//! test.eve().insert_mock_faction(1).await?;
//!
//! // Create HTTP mocks (side effect)
//! test.eve().create_faction_endpoint(factions, 1);
//! ```
//!
//! # Primary APIs
//!
//! - [`TestBuilder`] - Declarative builder for test setup (primary entry point)
//! - [`TestContext`] - Test context with access to database, ESI client, and fixtures
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
//! use bifrost_test_utils::{TestBuilder, factory};
//!
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
pub mod context;
pub mod error;
pub mod model;

// Internal modules (not exposed in public API)
mod fixtures;

// Re-export primary API types
pub use builder::TestBuilder;
pub use context::TestContext;
pub use error::TestError;

// Re-export factory module for creating mock data objects
pub use fixtures::eve::factory;

/// Prelude module containing commonly used imports for tests
///
/// # Usage
///
/// ```ignore
/// use bifrost_test_utils::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{builder::TestBuilder, context::TestContext, error::TestError, factory};
}

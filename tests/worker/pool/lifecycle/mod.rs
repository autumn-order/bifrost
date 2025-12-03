//! Tests for WorkerPool lifecycle management.
//!
//! This module verifies the behavior of the worker pool's lifecycle operations, including
//! starting and stopping the pool, checking running state, dispatcher count tracking,
//! idempotent operations, state transitions, and cleanup task coordination.

use super::*;

mod cleanup_task;
mod dispatcher_management;
mod start_stop;

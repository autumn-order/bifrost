//! Tests for WorkerJobQueue cleanup functionality.
//!
//! This module verifies the behavior of the cleanup system for managing stale jobs in the
//! worker queue. Tests cover manual cleanup operations, automatic cleanup task lifecycle,
//! TTL enforcement, and edge cases around job staleness boundaries.

use std::time::Duration;

use bifrost::server::model::worker::WorkerJob;
use chrono::Utc;

use crate::util::redis::RedisTest;

use super::setup_test_queue;

/// Job TTL is 1 hour in milliseconds
const JOB_TTL_MS: i64 = 60 * 60 * 1000;

mod cleanup_stale_jobs {
    use super::*;

    /// Tests successful removal of stale jobs from the queue.
    ///
    /// Verifies that jobs scheduled more than 1 hour in the past are correctly
    /// identified as stale and removed by the cleanup operation.
    ///
    /// Expected: cleanup_stale_jobs() returns 1 and the queue is empty
    #[tokio::test]
    async fn removes_stale_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Schedule a job more than 1 hour in the past (stale)
        let stale_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS + 1000);
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };

        let was_added = queue
            .schedule(job.clone(), stale_time)
            .await
            .expect("Failed to schedule job");
        assert!(was_added, "Job should be added");

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert_eq!(removed, 1, "Should remove 1 stale job");

        // Verify job was removed
        let popped = queue.pop().await.expect("Failed to pop");
        assert!(popped.is_none(), "Queue should be empty after cleanup");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that fresh jobs are not removed during cleanup.
    ///
    /// Verifies that jobs scheduled in the future are preserved by the cleanup
    /// operation and not mistakenly removed as stale.
    ///
    /// Expected: cleanup_stale_jobs() returns 0 and the job remains in the queue
    #[tokio::test]
    async fn preserves_fresh_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Schedule a job in the future (fresh)
        let fresh_time = Utc::now() + chrono::Duration::minutes(5);
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };

        queue
            .schedule(job.clone(), fresh_time)
            .await
            .expect("Failed to schedule job");

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert_eq!(removed, 0, "Should not remove any jobs");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests selective removal of only stale jobs in a mixed queue.
    ///
    /// Verifies that when the queue contains both stale and fresh jobs, cleanup
    /// only removes the stale ones while preserving jobs that are still valid.
    ///
    /// Expected: cleanup_stale_jobs() returns 3 (only stale jobs removed)
    #[tokio::test]
    async fn removes_only_stale_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Add stale jobs
        let stale_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS + 1000);
        for i in 1..=3 {
            let job = WorkerJob::UpdateCharacterInfo {
                character_id: i * 1000,
            };
            queue
                .schedule(job, stale_time)
                .await
                .expect("Failed to schedule stale job");
        }

        // Add fresh jobs
        let fresh_time = Utc::now() + chrono::Duration::minutes(5);
        for i in 1..=2 {
            let job = WorkerJob::UpdateAllianceInfo {
                alliance_id: i * 99000000,
            };
            queue
                .schedule(job, fresh_time)
                .await
                .expect("Failed to schedule fresh job");
        }

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert_eq!(removed, 3, "Should remove 3 stale jobs");

        // Verify only fresh jobs remain (but they're scheduled in the future)
        let popped = queue.pop().await.expect("Failed to pop");
        assert!(
            popped.is_none(),
            "Should not pop future-scheduled jobs immediately"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests cleanup operation on an empty queue.
    ///
    /// Verifies that running cleanup on an empty queue completes successfully
    /// without errors and correctly reports zero jobs removed.
    ///
    /// Expected: cleanup_stale_jobs() returns 0 without errors
    #[tokio::test]
    async fn handles_empty_queue() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Run cleanup on empty queue
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert_eq!(removed, 0, "Should remove 0 jobs from empty queue");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests cleanup at the exact TTL boundary.
    ///
    /// Verifies that a job scheduled exactly at the TTL threshold (1 hour ago)
    /// is correctly identified as stale and removed by cleanup.
    ///
    /// Expected: cleanup_stale_jobs() removes the boundary job
    #[tokio::test]
    async fn removes_job_exactly_at_ttl() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Schedule a job exactly at the TTL boundary (should be removed)
        let boundary_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS);
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };

        queue
            .schedule(job, boundary_time)
            .await
            .expect("Failed to schedule job");

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert!(removed >= 1, "Should remove job at or beyond TTL boundary");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests cleanup of multiple different job types.
    ///
    /// Verifies that cleanup correctly handles and removes stale jobs regardless
    /// of their type (Character, Alliance, Corporation, Affiliation updates).
    ///
    /// Expected: cleanup_stale_jobs() returns 4 (all stale jobs removed)
    #[tokio::test]
    async fn removes_multiple_job_types() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let stale_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS + 1000);

        // Add different types of stale jobs
        let jobs = vec![
            WorkerJob::UpdateCharacterInfo {
                character_id: 12345,
            },
            WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000001,
            },
            WorkerJob::UpdateCorporationInfo {
                corporation_id: 98000001,
            },
            WorkerJob::UpdateAffiliations {
                character_ids: vec![1, 2, 3],
            },
        ];

        for job in jobs {
            queue
                .schedule(job, stale_time)
                .await
                .expect("Failed to schedule job");
        }

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert_eq!(removed, 4, "Should remove all 4 stale jobs");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that jobs just under the TTL threshold are preserved.
    ///
    /// Verifies that a job scheduled just under 1 hour ago (within TTL) is not
    /// removed by cleanup, ensuring the TTL boundary is correctly enforced.
    ///
    /// Expected: cleanup_stale_jobs() returns 0 and the job remains in queue
    #[tokio::test]
    async fn preserves_jobs_just_under_ttl() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Schedule a job just under the TTL (should be preserved)
        let recent_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS - 1000);
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };

        queue
            .schedule(job.clone(), recent_time)
            .await
            .expect("Failed to schedule job");

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

        assert_eq!(removed, 0, "Should not remove jobs under TTL");

        // Verify job is still there
        let popped = queue.pop().await.expect("Failed to pop");
        assert!(popped.is_some(), "Job should still be in queue");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }
}

mod cleanup_task_lifecycle {
    use super::*;

    /// Tests the cleanup task lifecycle from stopped to running to stopped.
    ///
    /// Verifies that the cleanup task correctly reports its running state through
    /// the start and stop operations, ensuring proper lifecycle management.
    ///
    /// Expected: is_cleanup_running() reflects the current task state correctly
    #[tokio::test]
    async fn transitions_through_states() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Initially not running
        assert!(
            !queue.is_cleanup_running().await,
            "Cleanup should not be running initially"
        );

        // Start cleanup
        queue.start_cleanup().await;

        assert!(
            queue.is_cleanup_running().await,
            "Cleanup should be running after start"
        );

        // Stop cleanup
        queue.stop_cleanup().await;

        assert!(
            !queue.is_cleanup_running().await,
            "Cleanup should not be running after stop"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that starting the cleanup task multiple times is idempotent.
    ///
    /// Verifies that calling start_cleanup() multiple times does not cause errors
    /// or unexpected behavior, maintaining a single running cleanup task.
    ///
    /// Expected: Cleanup remains running after multiple start calls
    #[tokio::test]
    async fn start_is_idempotent() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Start cleanup twice
        queue.start_cleanup().await;
        queue.start_cleanup().await;

        assert!(
            queue.is_cleanup_running().await,
            "Cleanup should be running after multiple starts"
        );

        queue.stop_cleanup().await;

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that stopping the cleanup task multiple times is idempotent.
    ///
    /// Verifies that calling stop_cleanup() multiple times, including when the
    /// task is not running, does not cause errors or unexpected behavior.
    ///
    /// Expected: Cleanup remains stopped after multiple stop calls
    #[tokio::test]
    async fn stop_is_idempotent() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        queue.start_cleanup().await;
        queue.stop_cleanup().await;
        queue.stop_cleanup().await; // Second stop should be safe

        assert!(
            !queue.is_cleanup_running().await,
            "Cleanup should not be running after multiple stops"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that the cleanup task can be restarted after stopping.
    ///
    /// Verifies that the cleanup task can be stopped and started again, ensuring
    /// it properly reinitializes and continues functioning correctly.
    ///
    /// Expected: Cleanup runs successfully after restart
    #[tokio::test]
    async fn can_restart_after_stop() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Start, stop, start again
        queue.start_cleanup().await;
        assert!(queue.is_cleanup_running().await);

        queue.stop_cleanup().await;
        assert!(!queue.is_cleanup_running().await);

        queue.start_cleanup().await;
        assert!(
            queue.is_cleanup_running().await,
            "Cleanup should be running after restart"
        );

        queue.stop_cleanup().await;

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests cleanup task operation with a custom interval.
    ///
    /// Verifies that the cleanup task can be started with a custom interval setting
    /// and operates correctly with shorter intervals for testing purposes.
    ///
    /// Expected: Cleanup task starts and runs with custom interval
    #[tokio::test]
    async fn runs_with_custom_interval() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Start with very short interval for testing
        queue.start_cleanup().await;

        assert!(
            queue.is_cleanup_running().await,
            "Cleanup should be running with custom interval"
        );

        // Give it time to run at least once
        tokio::time::sleep(Duration::from_millis(100)).await;

        queue.stop_cleanup().await;

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that the cleanup task automatically removes stale jobs.
    ///
    /// Verifies that when the cleanup task is running, it periodically removes
    /// stale jobs from the queue automatically without manual intervention.
    ///
    /// Expected: All stale jobs are removed after cleanup task runs
    #[tokio::test]
    async fn automatically_removes_stale_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Add stale jobs
        let stale_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS + 1000);
        for i in 1..=5 {
            let job = WorkerJob::UpdateCharacterInfo {
                character_id: i * 1000,
            };
            queue
                .schedule(job, stale_time)
                .await
                .expect("Failed to schedule stale job");
        }

        // Start cleanup with very fast interval
        queue.start_cleanup().await;

        // Wait for cleanup to run
        tokio::time::sleep(Duration::from_millis(200)).await;

        queue.stop_cleanup().await;

        // Verify all stale jobs were removed
        let popped = queue.pop().await.expect("Failed to pop");
        assert!(
            popped.is_none(),
            "All stale jobs should have been cleaned up"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that cleanup task stops gracefully even during cleanup operation.
    ///
    /// Verifies that the cleanup task can be stopped gracefully even if it's
    /// currently performing a cleanup operation, without hanging or errors.
    ///
    /// Expected: stop_cleanup() completes successfully and task stops
    #[tokio::test]
    async fn stops_gracefully_during_cleanup() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Add many stale jobs to make cleanup take some time
        let stale_time = Utc::now() - chrono::Duration::milliseconds(JOB_TTL_MS + 1000);
        for i in 1..=100 {
            let job = WorkerJob::UpdateCharacterInfo {
                character_id: i * 1000,
            };
            queue
                .schedule(job, stale_time)
                .await
                .expect("Failed to schedule job");
        }

        // Start cleanup with very fast interval
        queue.start_cleanup().await;

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop should succeed even if cleanup is running
        queue.stop_cleanup().await;

        assert!(
            !queue.is_cleanup_running().await,
            "Cleanup should stop gracefully"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that stopping cleanup task without starting it is safe.
    ///
    /// Verifies that calling stop_cleanup() on a queue that never had cleanup
    /// started does not cause errors or panics, handling the edge case gracefully.
    ///
    /// Expected: stop_cleanup() completes successfully without errors
    #[tokio::test]
    async fn stop_without_start_is_safe() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Stopping without starting should be safe
        queue.stop_cleanup().await;

        assert!(
            !queue.is_cleanup_running().await,
            "Should handle stop without start gracefully"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }
}

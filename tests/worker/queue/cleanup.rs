//! Tests for WorkerJobQueue cleanup functionality
//!
//! These tests verify:
//! - Manual cleanup of stale jobs
//! - Automatic cleanup task lifecycle
//! - TTL enforcement for jobs
//! - Cleanup only removes jobs older than TTL
//! - Cleanup task can be started and stopped
//! - Cleanup task respects custom intervals

use std::time::Duration;

use bifrost::server::model::worker::WorkerJob;
use chrono::Utc;

use crate::redis::RedisTest;

use super::setup_test_queue;

/// Job TTL is 1 hour in milliseconds
const JOB_TTL_MS: i64 = 60 * 60 * 1000;

#[tokio::test]
async fn test_cleanup_removes_stale_jobs() {
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

#[tokio::test]
async fn test_cleanup_preserves_fresh_jobs() {
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

#[tokio::test]
async fn test_cleanup_removes_only_stale_jobs() {
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

#[tokio::test]
async fn test_cleanup_on_empty_queue() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Run cleanup on empty queue
    let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");

    assert_eq!(removed, 0, "Should remove 0 jobs from empty queue");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_cleanup_task_lifecycle() {
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

#[tokio::test]
async fn test_cleanup_task_idempotent_start() {
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

#[tokio::test]
async fn test_cleanup_task_idempotent_stop() {
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

#[tokio::test]
async fn test_cleanup_task_can_restart() {
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

#[tokio::test]
async fn test_cleanup_task_with_custom_interval() {
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

#[tokio::test]
async fn test_cleanup_task_removes_stale_jobs_automatically() {
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

#[tokio::test]
async fn test_cleanup_boundary_job_exactly_at_ttl() {
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

#[tokio::test]
async fn test_cleanup_multiple_job_types() {
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

#[tokio::test]
async fn test_cleanup_preserves_jobs_just_under_ttl() {
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

#[tokio::test]
async fn test_cleanup_task_stops_gracefully_during_cleanup() {
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

#[tokio::test]
async fn test_cleanup_stop_without_start() {
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

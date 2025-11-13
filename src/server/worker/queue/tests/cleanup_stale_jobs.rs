//! Tests for WorkerJobQueue::cleanup_stale_jobs method
//!
//! These tests verify the cleanup_stale_jobs method's behavior including:
//! - Removing stale jobs older than TTL
//! - Preserving recent jobs within TTL
//! - Handling empty queues
//! - Handling mixed old and new jobs
//! - Returning correct count of removed jobs
//! - Different job types are all cleaned up properly

use bifrost_test_utils::RedisTest;
use chrono::Utc;
use fred::prelude::*;

use super::super::JOB_TTL_MS;
use crate::server::{model::worker::WorkerJob, worker::queue::WorkerJobQueue};

async fn insert_job_with_timestamp(
    pool: &Pool,
    queue_name: &str,
    job: &WorkerJob,
    timestamp_ms: i64,
) -> Result<(), fred::error::Error> {
    let identity = job.identity().expect("Should generate identity");
    let score = timestamp_ms as f64;
    let _: () = pool
        .zadd(queue_name, None, None, false, false, (score, identity))
        .await?;
    Ok(())
}

/// Helper to get the count of jobs in the queue
async fn get_queue_size(pool: &Pool, queue_name: &str) -> Result<i64, fred::error::Error> {
    pool.zcard(queue_name).await
}

#[tokio::test]
async fn test_cleanup_empty_queue() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Cleanup on empty queue should succeed and return 0
    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed on empty queue");
    assert_eq!(result.unwrap(), 0, "Should remove 0 jobs from empty queue");
}

#[tokio::test]
async fn test_cleanup_removes_stale_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Insert job with timestamp older than TTL
    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job");

    // Verify job exists before cleanup
    let size_before = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_before, 1, "Queue should have 1 job before cleanup");

    // Run cleanup
    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 1, "Should remove 1 stale job");

    // Verify job was removed
    let size_after = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_after, 0, "Queue should be empty after cleanup");
}

#[tokio::test]
async fn test_cleanup_preserves_recent_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Push job normally (will have current timestamp)
    queue.push(job.clone()).await.expect("Should push job");

    // Verify job exists before cleanup
    let size_before = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_before, 1, "Queue should have 1 job before cleanup");

    // Run cleanup
    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 0, "Should not remove recent jobs");

    // Verify job still exists
    let size_after = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_after, 1, "Queue should still have 1 job after cleanup");
}

#[tokio::test]
async fn test_cleanup_removes_multiple_stale_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;

    // Insert multiple stale jobs of different types
    let job1 = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let job2 = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };
    let job3 = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job1,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job 1");
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job2,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job 2");
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job3,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job 3");

    // Verify all jobs exist
    let size_before = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_before, 3, "Queue should have 3 jobs before cleanup");

    // Run cleanup
    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 3, "Should remove all 3 stale jobs");

    // Verify all jobs were removed
    let size_after = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_after, 0, "Queue should be empty after cleanup");
}

#[tokio::test]
async fn test_cleanup_handles_mixed_stale_and_recent_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    let recent_timestamp = Utc::now().timestamp_millis();

    // Insert stale jobs
    let stale_job1 = WorkerJob::UpdateCharacterInfo {
        character_id: 11111,
    };
    let stale_job2 = WorkerJob::UpdateCharacterInfo {
        character_id: 22222,
    };

    // Insert recent jobs
    let recent_job1 = WorkerJob::UpdateCharacterInfo {
        character_id: 33333,
    };
    let recent_job2 = WorkerJob::UpdateCharacterInfo {
        character_id: 44444,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job1,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job 1");
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job2,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job 2");
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &recent_job1,
        recent_timestamp,
    )
    .await
    .expect("Should insert recent job 1");
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &recent_job2,
        recent_timestamp,
    )
    .await
    .expect("Should insert recent job 2");

    // Verify all jobs exist
    let size_before = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_before, 4, "Queue should have 4 jobs before cleanup");

    // Run cleanup
    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 2, "Should remove 2 stale jobs");

    // Verify only recent jobs remain
    let size_after = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(
        size_after, 2,
        "Queue should have 2 recent jobs after cleanup"
    );
}

#[tokio::test]
async fn test_cleanup_removes_stale_alliance_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 5000;

    let job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale alliance job");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 1, "Should remove stale alliance job");
}

#[tokio::test]
async fn test_cleanup_removes_stale_corporation_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 5000;

    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale corporation job");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 1, "Should remove stale corporation job");
}

#[tokio::test]
async fn test_cleanup_removes_stale_affiliation_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 5000;

    let job = WorkerJob::UpdateAffiliations {
        character_ids: vec![12345, 67890, 11111],
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale affiliation job");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 1, "Should remove stale affiliation job");
}

#[tokio::test]
async fn test_cleanup_on_boundary_exact_ttl() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Job exactly at TTL boundary (should be removed)
    let boundary_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS;

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        boundary_timestamp,
    )
    .await
    .expect("Should insert job at boundary");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(
        result.unwrap(),
        1,
        "Should remove job at exact TTL boundary"
    );
}

#[tokio::test]
async fn test_cleanup_just_inside_ttl() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Job just inside TTL (1 second before expiry, should be preserved)
    let inside_ttl_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS + 1000;

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        inside_ttl_timestamp,
    )
    .await
    .expect("Should insert job inside TTL");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 0, "Should not remove job inside TTL");

    let size = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size, 1, "Job should still exist after cleanup");
}

#[tokio::test]
async fn test_cleanup_multiple_times() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        stale_timestamp,
    )
    .await
    .expect("Should insert stale job");

    // First cleanup should remove the job
    let result1 = queue.cleanup_stale_jobs().await;
    assert!(result1.is_ok(), "First cleanup should succeed");
    assert_eq!(result1.unwrap(), 1, "First cleanup should remove 1 job");

    // Second cleanup should find nothing
    let result2 = queue.cleanup_stale_jobs().await;
    assert!(result2.is_ok(), "Second cleanup should succeed");
    assert_eq!(result2.unwrap(), 0, "Second cleanup should remove 0 jobs");
}

#[tokio::test]
async fn test_cleanup_with_very_old_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Job from 7 days ago (way past TTL)
    let very_old_timestamp = Utc::now().timestamp_millis() - (7 * 24 * 60 * 60 * 1000);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &job,
        very_old_timestamp,
    )
    .await
    .expect("Should insert very old job");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 1, "Should remove very old job");
}

#[tokio::test]
async fn test_cleanup_large_batch_of_stale_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;

    // Insert 100 stale jobs
    for i in 1..=100 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &job,
            stale_timestamp,
        )
        .await
        .expect("Should insert stale job");
    }

    let size_before = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(
        size_before, 100,
        "Queue should have 100 jobs before cleanup"
    );

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(result.unwrap(), 100, "Should remove all 100 stale jobs");

    let size_after = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_after, 0, "Queue should be empty after cleanup");
}

#[tokio::test]
async fn test_cleanup_with_gradual_age_distribution() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let now = Utc::now().timestamp_millis();

    // Insert jobs with timestamps at various ages
    // 2 very stale (48 hours old)
    // 2 slightly stale (25 hours old)
    // 2 at boundary (24 hours old)
    // 2 recent (12 hours old)
    // 2 very recent (1 hour old)

    for i in 0..2 {
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 10000 + i,
        };
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &job,
            now - (48 * 60 * 60 * 1000),
        )
        .await
        .expect("Should insert very stale job");
    }

    for i in 0..2 {
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 20000 + i,
        };
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &job,
            now - (25 * 60 * 60 * 1000),
        )
        .await
        .expect("Should insert slightly stale job");
    }

    for i in 0..2 {
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 30000 + i,
        };
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &job,
            now - JOB_TTL_MS,
        )
        .await
        .expect("Should insert boundary job");
    }

    for i in 0..2 {
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 40000 + i,
        };
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &job,
            now - (12 * 60 * 60 * 1000),
        )
        .await
        .expect("Should insert recent job");
    }

    for i in 0..2 {
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 50000 + i,
        };
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &job,
            now - (1 * 60 * 60 * 1000),
        )
        .await
        .expect("Should insert very recent job");
    }

    let size_before = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_before, 10, "Queue should have 10 jobs before cleanup");

    let result = queue.cleanup_stale_jobs().await;
    assert!(result.is_ok(), "Cleanup should succeed");
    assert_eq!(
        result.unwrap(),
        6,
        "Should remove 6 stale jobs (2 very stale + 2 slightly stale + 2 at boundary)"
    );

    let size_after = get_queue_size(&redis.redis_pool, &redis.queue_name())
        .await
        .expect("Should get queue size");
    assert_eq!(size_after, 4, "Queue should have 4 recent jobs remaining");
}

//! Tests for WorkerJobQueue periodic cleanup functionality
//!
//! These tests verify that the automatic cleanup mechanism works correctly:
//! - Cleanup is triggered after CLEANUP_INTERVAL (1000) job additions
//! - Cleanup only runs when jobs are successfully added (not for duplicates)
//! - Cleanup runs in the background without blocking
//! - Stale jobs are removed during periodic cleanup

use bifrost_test_utils::RedisTest;
use chrono::{Duration, Utc};
use fred::prelude::*;

use crate::server::{
    model::worker::WorkerJob,
    worker::queue::{WorkerJobQueue, JOB_TTL_MS},
};

/// Helper function to insert a job with a specific timestamp directly into Redis
async fn insert_job_with_timestamp(
    pool: &Pool,
    queue_name: &str,
    job: &WorkerJob,
    timestamp_ms: i64,
) {
    let identity = job.identity().expect("Should generate identity");
    let score = timestamp_ms as f64;
    let _: () = pool
        .zadd(queue_name, None, None, false, false, (score, identity))
        .await
        .expect("Should add job to Redis");
}

/// Helper function to get the count of jobs in the queue
async fn get_queue_size(pool: &Pool, queue_name: &str) -> usize {
    let count: i64 = pool.zcard(queue_name).await.expect("Should get count");
    count as usize
}

#[tokio::test]
async fn test_periodic_cleanup_not_triggered_before_interval() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert a stale job directly
    let stale_job = WorkerJob::UpdateCharacterInfo {
        character_id: 99999,
    };
    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job,
        stale_timestamp,
    )
    .await;

    // Push jobs but not enough to trigger cleanup (less than 1000)
    for i in 1..=100 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Give a small delay for any potential background cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Stale job should still be present (cleanup not triggered)
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 101,
        "Stale job should still be present when cleanup interval not reached"
    );
}

#[tokio::test]
async fn test_periodic_cleanup_triggered_after_interval() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert a stale job directly
    let stale_job = WorkerJob::UpdateCharacterInfo {
        character_id: 99999,
    };
    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job,
        stale_timestamp,
    )
    .await;

    // Push exactly 1000 jobs to trigger cleanup
    for i in 1..=1000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Give time for background cleanup to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Stale job should be removed by periodic cleanup
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 1000,
        "Stale job should be removed after cleanup interval reached"
    );
}

#[tokio::test]
async fn test_periodic_cleanup_only_counts_successful_additions() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert a stale job directly
    let stale_job = WorkerJob::UpdateCharacterInfo {
        character_id: 99999,
    };
    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job,
        stale_timestamp,
    )
    .await;

    // Push 500 unique jobs
    for i in 1..=500 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Try to push 1000 duplicates (these should not increment the counter)
    for i in 1..=1000 {
        let job = WorkerJob::UpdateCharacterInfo {
            character_id: i % 500 + 1, // Reuse IDs from 1-500
        };
        let result = queue.push(job).await.expect("Should push successfully");
        assert!(!result, "Should be detected as duplicate");
    }

    // Give a small delay for any potential background cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Stale job should still be present (only 500 successful additions)
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 501,
        "Stale job should still be present - duplicates don't count toward cleanup interval"
    );

    // Now push 500 more unique jobs to reach 1000 total successful additions
    for i in 501..=1000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Give time for background cleanup to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Stale job should now be removed
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 1000,
        "Stale job should be removed after 1000 successful additions"
    );
}

#[tokio::test]
async fn test_periodic_cleanup_triggers_multiple_times() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert first batch of stale jobs
    for i in 1..=10 {
        let stale_job = WorkerJob::UpdateCharacterInfo {
            character_id: 100000 + i,
        };
        let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &stale_job,
            stale_timestamp,
        )
        .await;
    }

    // Push 1000 jobs to trigger first cleanup
    for i in 1..=1000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Give time for first cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Insert second batch of stale jobs
    for i in 11..=20 {
        let stale_job = WorkerJob::UpdateCharacterInfo {
            character_id: 100000 + i,
        };
        let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
        insert_job_with_timestamp(
            &redis.redis_pool,
            &redis.queue_name(),
            &stale_job,
            stale_timestamp,
        )
        .await;
    }

    // Push another 1000 jobs to trigger second cleanup
    for i in 1001..=2000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Give time for second cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // All stale jobs should be removed, only the 2000 recent jobs should remain
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 2000,
        "All stale jobs should be removed after second cleanup"
    );
}

#[tokio::test]
async fn test_periodic_cleanup_works_with_schedule() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert a stale job directly
    let stale_job = WorkerJob::UpdateCharacterInfo {
        character_id: 99999,
    };
    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job,
        stale_timestamp,
    )
    .await;

    let schedule_time = Utc::now() + Duration::minutes(5);

    // Schedule 1000 jobs to trigger cleanup
    for i in 1..=1000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue
            .schedule(job, schedule_time)
            .await
            .expect("Should schedule successfully");
    }

    // Give time for background cleanup to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Stale job should be removed by periodic cleanup triggered by schedule
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 1000,
        "Stale job should be removed after cleanup interval reached via schedule"
    );
}

#[tokio::test]
async fn test_periodic_cleanup_shared_counter_between_push_and_schedule() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert a stale job directly
    let stale_job = WorkerJob::UpdateCharacterInfo {
        character_id: 99999,
    };
    let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;
    insert_job_with_timestamp(
        &redis.redis_pool,
        &redis.queue_name(),
        &stale_job,
        stale_timestamp,
    )
    .await;

    let schedule_time = Utc::now() + Duration::minutes(5);

    // Mix of push and schedule operations totaling 1000
    for i in 1..=500 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    for i in 501..=1000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue
            .schedule(job, schedule_time)
            .await
            .expect("Should schedule successfully");
    }

    // Give time for background cleanup to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Stale job should be removed (counter is shared between push and schedule)
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 1000,
        "Stale job should be removed - counter is shared between push and schedule"
    );
}

#[tokio::test]
async fn test_periodic_cleanup_removes_multiple_stale_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Insert 50 stale jobs of different types
    for i in 1..=50 {
        let stale_timestamp = Utc::now().timestamp_millis() - JOB_TTL_MS - 1000;

        match i % 4 {
            0 => {
                let job = WorkerJob::UpdateCharacterInfo {
                    character_id: 100000 + i,
                };
                insert_job_with_timestamp(
                    &redis.redis_pool,
                    &redis.queue_name(),
                    &job,
                    stale_timestamp,
                )
                .await;
            }
            1 => {
                let job = WorkerJob::UpdateAllianceInfo {
                    alliance_id: 100000 + i,
                };
                insert_job_with_timestamp(
                    &redis.redis_pool,
                    &redis.queue_name(),
                    &job,
                    stale_timestamp,
                )
                .await;
            }
            2 => {
                let job = WorkerJob::UpdateCorporationInfo {
                    corporation_id: 100000 + i,
                };
                insert_job_with_timestamp(
                    &redis.redis_pool,
                    &redis.queue_name(),
                    &job,
                    stale_timestamp,
                )
                .await;
            }
            _ => {
                let job = WorkerJob::UpdateAffiliations {
                    character_ids: vec![100000 + i, 200000 + i],
                };
                insert_job_with_timestamp(
                    &redis.redis_pool,
                    &redis.queue_name(),
                    &job,
                    stale_timestamp,
                )
                .await;
            }
        }
    }

    // Push 1000 jobs to trigger cleanup
    for i in 1..=1000 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Should push successfully");
    }

    // Give time for background cleanup to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // All 50 stale jobs should be removed
    let size = get_queue_size(&redis.redis_pool, &redis.queue_name()).await;
    assert_eq!(
        size, 1000,
        "All 50 stale jobs should be removed by periodic cleanup"
    );
}

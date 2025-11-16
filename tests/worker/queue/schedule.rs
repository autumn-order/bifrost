//! Tests for WorkerJobQueue::schedule method
//!
//! These tests verify the schedule method's behavior including:
//! - Successfully scheduling jobs at future times
//! - Preventing duplicate scheduled jobs
//! - Handling different job types (Character, Alliance, Corporation, Affiliation)
//! - Validation errors (empty/oversized affiliation batches)
//! - Duplicate detection for affiliation jobs (order-independent)
//! - Timestamp storage verification
//! - Scheduling multiple jobs at different times

use bifrost::server::model::worker::WorkerJob;
use chrono::{Duration, Utc};
use fred::interfaces::SortedSetsInterface;

use crate::redis::RedisTest;

use super::setup_test_queue;

static MAX_AFFILIATION_BATCH_SIZE: i64 = 1000;

#[tokio::test]
async fn test_schedule_new_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let schedule_time = Utc::now() + Duration::minutes(5);

    let result = queue.schedule(job.clone(), schedule_time).await;
    assert!(result.is_ok(), "Schedule should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_duplicate_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let schedule_time_1 = Utc::now() + Duration::minutes(5);
    let schedule_time_2 = Utc::now() + Duration::minutes(10);

    // Schedule first time
    let result1 = queue.schedule(job.clone(), schedule_time_1).await;
    assert!(result1.is_ok(), "First schedule should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Schedule duplicate at different time
    let result2 = queue.schedule(job.clone(), schedule_time_2).await;
    assert!(
        result2.is_ok(),
        "Duplicate schedule should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_new_alliance_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };
    let schedule_time = Utc::now() + Duration::minutes(15);

    let result = queue.schedule(job.clone(), schedule_time).await;
    assert!(result.is_ok(), "Schedule should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_duplicate_alliance_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };
    let schedule_time_1 = Utc::now() + Duration::minutes(5);
    let schedule_time_2 = Utc::now() + Duration::minutes(10);

    // Schedule first time
    let result1 = queue.schedule(job.clone(), schedule_time_1).await;
    assert!(result1.is_ok(), "First schedule should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Schedule duplicate
    let result2 = queue.schedule(job.clone(), schedule_time_2).await;
    assert!(
        result2.is_ok(),
        "Duplicate schedule should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_new_corporation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };
    let schedule_time = Utc::now() + Duration::minutes(20);

    let result = queue.schedule(job.clone(), schedule_time).await;
    assert!(result.is_ok(), "Schedule should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_duplicate_corporation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };
    let schedule_time_1 = Utc::now() + Duration::minutes(5);
    let schedule_time_2 = Utc::now() + Duration::minutes(10);

    // Schedule first time
    let result1 = queue.schedule(job.clone(), schedule_time_1).await;
    assert!(result1.is_ok(), "First schedule should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Schedule duplicate
    let result2 = queue.schedule(job.clone(), schedule_time_2).await;
    assert!(
        result2.is_ok(),
        "Duplicate schedule should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_affiliation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let character_ids = vec![12345, 67890, 11111, 22222, 33333];
    let job = WorkerJob::UpdateAffiliations { character_ids };
    let schedule_time = Utc::now() + Duration::minutes(10);

    let result = queue.schedule(job.clone(), schedule_time).await;
    assert!(result.is_ok(), "Schedule should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_duplicate_affiliation_job_same_ids() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let character_ids = vec![12345, 67890, 11111, 22222, 33333];
    let job = WorkerJob::UpdateAffiliations {
        character_ids: character_ids.clone(),
    };
    let schedule_time_1 = Utc::now() + Duration::minutes(5);
    let schedule_time_2 = Utc::now() + Duration::minutes(10);

    // Schedule first time
    let result1 = queue.schedule(job.clone(), schedule_time_1).await;
    assert!(result1.is_ok(), "First schedule should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Schedule duplicate with same IDs
    let job2 = WorkerJob::UpdateAffiliations { character_ids };
    let result2 = queue.schedule(job2, schedule_time_2).await;
    assert!(
        result2.is_ok(),
        "Duplicate schedule should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_affiliation_job_different_order_is_duplicate() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let character_ids_1 = vec![12345, 67890, 11111];
    let character_ids_2 = vec![67890, 11111, 12345]; // Same IDs, different order

    let job1 = WorkerJob::UpdateAffiliations {
        character_ids: character_ids_1,
    };
    let job2 = WorkerJob::UpdateAffiliations {
        character_ids: character_ids_2,
    };
    let schedule_time_1 = Utc::now() + Duration::minutes(5);
    let schedule_time_2 = Utc::now() + Duration::minutes(10);

    // Schedule first job
    let result1 = queue.schedule(job1, schedule_time_1).await;
    assert!(result1.is_ok(), "First schedule should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Schedule second job with same IDs in different order
    let result2 = queue.schedule(job2, schedule_time_2).await;
    assert!(
        result2.is_ok(),
        "Second schedule should succeed (but not add)"
    );
    assert_eq!(
        result2.unwrap(),
        false,
        "Job with same IDs in different order should be detected as duplicate"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_affiliation_job_different_ids_not_duplicate() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let character_ids_1 = vec![12345, 67890, 11111];
    let character_ids_2 = vec![99999, 88888, 77777]; // Different IDs

    let job1 = WorkerJob::UpdateAffiliations {
        character_ids: character_ids_1,
    };
    let job2 = WorkerJob::UpdateAffiliations {
        character_ids: character_ids_2,
    };
    let schedule_time_1 = Utc::now() + Duration::minutes(5);
    let schedule_time_2 = Utc::now() + Duration::minutes(10);

    // Schedule first job
    let result1 = queue.schedule(job1, schedule_time_1).await;
    assert!(result1.is_ok(), "First schedule should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Schedule second job with different IDs
    let result2 = queue.schedule(job2, schedule_time_2).await;
    assert!(result2.is_ok(), "Second schedule should succeed");
    assert_eq!(
        result2.unwrap(),
        true,
        "Job with different IDs should be added"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_affiliation_job_empty_ids_fails() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateAffiliations {
        character_ids: vec![],
    };
    let schedule_time = Utc::now() + Duration::minutes(5);

    let result = queue.schedule(job, schedule_time).await;
    assert!(result.is_err(), "Schedule with empty IDs should fail");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_affiliation_job_too_many_ids_fails() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Create a batch larger than MAX_AFFILIATION_BATCH_SIZE
    let character_ids: Vec<i64> = (1..=(MAX_AFFILIATION_BATCH_SIZE + 1) as i64).collect();
    let job = WorkerJob::UpdateAffiliations { character_ids };
    let schedule_time = Utc::now() + Duration::minutes(5);

    let result = queue.schedule(job, schedule_time).await;
    assert!(result.is_err(), "Schedule with too many IDs should fail");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_affiliation_job_max_size_succeeds() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Create a batch exactly at MAX_AFFILIATION_BATCH_SIZE
    let character_ids: Vec<i64> = (1..=MAX_AFFILIATION_BATCH_SIZE as i64).collect();
    let job = WorkerJob::UpdateAffiliations { character_ids };
    let schedule_time = Utc::now() + Duration::minutes(5);

    let result = queue.schedule(job, schedule_time).await;
    assert!(result.is_ok(), "Schedule with max size should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_multiple_different_job_types() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job1 = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let job2 = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };
    let job3 = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };
    let job4 = WorkerJob::UpdateAffiliations {
        character_ids: vec![11111, 22222],
    };

    let schedule_time = Utc::now() + Duration::minutes(5);

    let result1 = queue.schedule(job1, schedule_time).await;
    let result2 = queue.schedule(job2, schedule_time).await;
    let result3 = queue.schedule(job3, schedule_time).await;
    let result4 = queue.schedule(job4, schedule_time).await;

    assert!(
        result1.is_ok() && result1.unwrap(),
        "Character job should be added"
    );
    assert!(
        result2.is_ok() && result2.unwrap(),
        "Alliance job should be added"
    );
    assert!(
        result3.is_ok() && result3.unwrap(),
        "Corporation job should be added"
    );
    assert!(
        result4.is_ok() && result4.unwrap(),
        "Affiliation job should be added"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_same_id_different_job_types_not_duplicate() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Same ID (12345) but different job types should not be considered duplicates
    let job1 = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let job2 = WorkerJob::UpdateAllianceInfo { alliance_id: 12345 };
    let job3 = WorkerJob::UpdateCorporationInfo {
        corporation_id: 12345,
    };

    let schedule_time = Utc::now() + Duration::minutes(5);

    let result1 = queue.schedule(job1, schedule_time).await;
    let result2 = queue.schedule(job2, schedule_time).await;
    let result3 = queue.schedule(job3, schedule_time).await;

    assert!(
        result1.is_ok() && result1.unwrap(),
        "Character job should be added"
    );
    assert!(
        result2.is_ok() && result2.unwrap(),
        "Alliance job should be added"
    );
    assert!(
        result3.is_ok() && result3.unwrap(),
        "Corporation job should be added"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_multiple_jobs_at_different_times() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Schedule multiple jobs at staggered times
    for i in 1..=10 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        let schedule_time = Utc::now() + Duration::minutes(i * 5);
        let result = queue.schedule(job, schedule_time).await;
        assert!(
            result.is_ok() && result.unwrap(),
            "Job {} should be added",
            i
        );
    }

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_stores_with_correct_timestamp() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let schedule_time = Utc::now() + Duration::minutes(30);
    let expected_timestamp = schedule_time.timestamp_millis();

    let result = queue.schedule(job.clone(), schedule_time).await;
    assert!(result.is_ok() && result.unwrap(), "Job should be added");

    // Verify job was stored with the correct timestamp
    let identity = job.identity().expect("Should generate identity");
    let score: Option<f64> = redis
        .redis_pool
        .zscore(&redis.queue_name(), &identity)
        .await
        .expect("Should get score");

    assert!(score.is_some(), "Job should have a score in Redis");
    let score_ms = score.unwrap() as i64;
    assert_eq!(
        score_ms, expected_timestamp,
        "Score should match scheduled timestamp"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_past_time_is_allowed() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    // Schedule in the past (e.g., for handling overflowed jobs from previous window)
    let schedule_time = Utc::now() - Duration::minutes(5);

    let result = queue.schedule(job.clone(), schedule_time).await;
    assert!(
        result.is_ok() && result.unwrap(),
        "Job should be added even if scheduled in the past"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_schedule_and_push_same_job_are_duplicates() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Push job first
    let result1 = queue.push(job.clone()).await;
    assert!(result1.is_ok() && result1.unwrap(), "Push should succeed");

    // Try to schedule the same job
    let schedule_time = Utc::now() + Duration::minutes(10);
    let result2 = queue.schedule(job.clone(), schedule_time).await;
    assert!(
        result2.is_ok() && !result2.unwrap(),
        "Schedule should detect duplicate from push"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_and_schedule_same_job_are_duplicates() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Schedule job first
    let schedule_time = Utc::now() + Duration::minutes(10);
    let result1 = queue.schedule(job.clone(), schedule_time).await;
    assert!(
        result1.is_ok() && result1.unwrap(),
        "Schedule should succeed"
    );

    // Try to push the same job
    let result2 = queue.push(job.clone()).await;
    assert!(
        result2.is_ok() && !result2.unwrap(),
        "Push should detect duplicate from schedule"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

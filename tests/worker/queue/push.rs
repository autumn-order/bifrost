//! Tests for WorkerJobQueue::push method
//!
//! These tests verify the push method's behavior including:
//! - Successfully pushing new jobs
//! - Preventing duplicate jobs (exact matches only)
//! - Handling different job types (Character, Alliance, Corporation, Affiliation)
//! - Timestamp storage verification
//! - Multiple job handling

use bifrost::server::model::worker::WorkerJob;
use chrono::Utc;
use fred::interfaces::SortedSetsInterface;

use crate::redis::RedisTest;

use super::setup_test_queue;

static MAX_AFFILIATION_BATCH_SIZE: i64 = 1000;

#[tokio::test]
async fn test_push_new_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    let result = queue.push(job.clone()).await;
    assert!(result.is_ok(), "Push should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_duplicate_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Push first time
    let result1 = queue.push(job.clone()).await;
    assert!(result1.is_ok(), "First push should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Push duplicate
    let result2 = queue.push(job.clone()).await;
    assert!(
        result2.is_ok(),
        "Duplicate push should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_new_alliance_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };

    let result = queue.push(job.clone()).await;
    assert!(result.is_ok(), "Push should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_duplicate_alliance_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };

    // Push first time
    let result1 = queue.push(job.clone()).await;
    assert!(result1.is_ok(), "First push should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Push duplicate
    let result2 = queue.push(job.clone()).await;
    assert!(
        result2.is_ok(),
        "Duplicate push should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_new_corporation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };

    let result = queue.push(job.clone()).await;
    assert!(result.is_ok(), "Push should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_duplicate_corporation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };

    // Push first time
    let result1 = queue.push(job.clone()).await;
    assert!(result1.is_ok(), "First push should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Push duplicate
    let result2 = queue.push(job.clone()).await;
    assert!(
        result2.is_ok(),
        "Duplicate push should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_affiliation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let character_ids = vec![12345, 67890, 11111, 22222, 33333];
    let job = WorkerJob::UpdateAffiliations { character_ids };

    let result = queue.push(job.clone()).await;
    assert!(result.is_ok(), "Push should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_duplicate_affiliation_job_same_ids() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let character_ids = vec![12345, 67890, 11111, 22222, 33333];
    let job = WorkerJob::UpdateAffiliations {
        character_ids: character_ids.clone(),
    };

    // Push first time
    let result1 = queue.push(job.clone()).await;
    assert!(result1.is_ok(), "First push should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Push duplicate with same IDs
    let job2 = WorkerJob::UpdateAffiliations { character_ids };
    let result2 = queue.push(job2).await;
    assert!(
        result2.is_ok(),
        "Duplicate push should succeed (but not add)"
    );
    assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_affiliation_job_different_ids_not_duplicate() {
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

    // Push first job
    let result1 = queue.push(job1).await;
    assert!(result1.is_ok(), "First push should succeed");
    assert_eq!(result1.unwrap(), true, "First job should be added");

    // Push second job with different IDs
    let result2 = queue.push(job2).await;
    assert!(result2.is_ok(), "Second push should succeed");
    assert_eq!(
        result2.unwrap(),
        true,
        "Job with different IDs should be added"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_affiliation_job_max_size_succeeds() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Create a batch exactly at MAX_AFFILIATION_BATCH_SIZE
    let character_ids: Vec<i64> = (1..=MAX_AFFILIATION_BATCH_SIZE as i64).collect();
    let job = WorkerJob::UpdateAffiliations { character_ids };

    let result = queue.push(job).await;
    assert!(result.is_ok(), "Push with max size should succeed");
    assert_eq!(result.unwrap(), true, "Job should be added");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_multiple_different_job_types() {
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

    let result1 = queue.push(job1).await;
    let result2 = queue.push(job2).await;
    let result3 = queue.push(job3).await;
    let result4 = queue.push(job4).await;

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
async fn test_push_same_id_different_job_types_not_duplicate() {
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

    let result1 = queue.push(job1).await;
    let result2 = queue.push(job2).await;
    let result3 = queue.push(job3).await;

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
async fn test_push_multiple_jobs_successfully() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    // Push multiple jobs to verify they all get added successfully
    for i in 1..=10 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        let result = queue.push(job).await;
        assert!(
            result.is_ok() && result.unwrap(),
            "Job {} should be added",
            i
        );
    }

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_push_stores_with_correct_timestamp() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let before = Utc::now().timestamp_millis();

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    let result = queue.push(job.clone()).await;
    assert!(result.is_ok() && result.unwrap(), "Job should be added");

    let after = Utc::now().timestamp_millis();

    // Verify job was stored with a timestamp in the correct range
    let serialized = serde_json::to_string(&job).expect("Should serialize job");
    let score: Option<f64> = redis
        .redis_pool
        .zscore(&redis.queue_name(), &serialized)
        .await
        .expect("Should get score");

    assert!(score.is_some(), "Job should have a score in Redis");
    let score_ms = score.unwrap() as i64;
    assert!(
        score_ms >= before && score_ms <= after,
        "Score should be between {} and {}, got {}",
        before,
        after,
        score_ms
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

//! Tests for WorkerJobQueue::get_all_of_type method
//!
//! These tests verify the get_all_of_type method's behavior including:
//! - Retrieving jobs from an empty queue
//! - Retrieving jobs of specific types (Character, Alliance, Corporation, Affiliation)
//! - Filtering jobs by type (no cross-contamination)
//! - Preserving scheduled times in returned jobs
//! - Not removing jobs from the queue (read-only operation)
//! - Handling mixed job types

use bifrost_test_utils::RedisTest;
use chrono::{Duration, Utc};

use crate::server::{model::worker::WorkerJob, worker::queue::WorkerJobQueue};

#[tokio::test]
async fn test_get_all_of_type_empty_queue() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Query for alliance jobs when queue is empty
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateAllianceInfo { alliance_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 0, "Expected no jobs in empty queue");
}

#[tokio::test]
async fn test_get_all_of_type_character_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push multiple character jobs
    let character_ids = vec![100, 200, 300];
    for id in &character_ids {
        let job = WorkerJob::UpdateCharacterInfo { character_id: *id };
        queue.push(job).await.expect("Failed to push job");
    }

    // Push some alliance jobs (should not be returned)
    queue
        .push(WorkerJob::UpdateAllianceInfo { alliance_id: 999 })
        .await
        .expect("Failed to push alliance job");

    // Query for character jobs
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateCharacterInfo { character_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 3, "Expected 3 character jobs");

    // Verify all jobs are character jobs with correct IDs
    let mut retrieved_ids: Vec<i64> = jobs
        .iter()
        .map(|qj| match qj.job {
            WorkerJob::UpdateCharacterInfo { character_id } => character_id,
            _ => panic!("Expected UpdateCharacterInfo job"),
        })
        .collect();
    retrieved_ids.sort();

    assert_eq!(retrieved_ids, character_ids, "Retrieved IDs should match");
}

#[tokio::test]
async fn test_get_all_of_type_alliance_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push multiple alliance jobs
    let alliance_ids = vec![1000, 2000, 3000, 4000];
    for id in &alliance_ids {
        let job = WorkerJob::UpdateAllianceInfo { alliance_id: *id };
        queue.push(job).await.expect("Failed to push job");
    }

    // Push some character jobs (should not be returned)
    queue
        .push(WorkerJob::UpdateCharacterInfo { character_id: 999 })
        .await
        .expect("Failed to push character job");

    // Query for alliance jobs
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateAllianceInfo { alliance_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 4, "Expected 4 alliance jobs");

    // Verify all jobs are alliance jobs with correct IDs
    let mut retrieved_ids: Vec<i64> = jobs
        .iter()
        .map(|qj| match qj.job {
            WorkerJob::UpdateAllianceInfo { alliance_id } => alliance_id,
            _ => panic!("Expected UpdateAllianceInfo job"),
        })
        .collect();
    retrieved_ids.sort();

    assert_eq!(retrieved_ids, alliance_ids, "Retrieved IDs should match");
}

#[tokio::test]
async fn test_get_all_of_type_corporation_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push multiple corporation jobs
    let corporation_ids = vec![5000, 6000, 7000];
    for id in &corporation_ids {
        let job = WorkerJob::UpdateCorporationInfo {
            corporation_id: *id,
        };
        queue.push(job).await.expect("Failed to push job");
    }

    // Push some other job types (should not be returned)
    queue
        .push(WorkerJob::UpdateAllianceInfo { alliance_id: 999 })
        .await
        .expect("Failed to push alliance job");

    // Query for corporation jobs
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateCorporationInfo { corporation_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 3, "Expected 3 corporation jobs");

    // Verify all jobs are corporation jobs with correct IDs
    let mut retrieved_ids: Vec<i64> = jobs
        .iter()
        .map(|qj| match qj.job {
            WorkerJob::UpdateCorporationInfo { corporation_id } => corporation_id,
            _ => panic!("Expected UpdateCorporationInfo job"),
        })
        .collect();
    retrieved_ids.sort();

    assert_eq!(retrieved_ids, corporation_ids, "Retrieved IDs should match");
}

#[tokio::test]
async fn test_get_all_of_type_affiliation_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push multiple affiliation batch jobs
    let batch1 = vec![1, 2, 3];
    let batch2 = vec![4, 5, 6];
    let batch3 = vec![7, 8, 9];

    queue
        .push(WorkerJob::UpdateAffiliations {
            character_ids: batch1.clone(),
        })
        .await
        .expect("Failed to push batch 1");
    queue
        .push(WorkerJob::UpdateAffiliations {
            character_ids: batch2.clone(),
        })
        .await
        .expect("Failed to push batch 2");
    queue
        .push(WorkerJob::UpdateAffiliations {
            character_ids: batch3.clone(),
        })
        .await
        .expect("Failed to push batch 3");

    // Push some other job types (should not be returned)
    queue
        .push(WorkerJob::UpdateCharacterInfo { character_id: 999 })
        .await
        .expect("Failed to push character job");

    // Query for affiliation jobs
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateAffiliations {
            character_ids: Vec::new(),
        })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 3, "Expected 3 affiliation batch jobs");

    // Verify all jobs are affiliation jobs
    // Note: character_ids will be empty since they're not stored in identity
    for qj in &jobs {
        match &qj.job {
            WorkerJob::UpdateAffiliations { character_ids } => {
                assert_eq!(
                    character_ids.len(),
                    0,
                    "Character IDs should be empty in retrieved affiliation jobs"
                );
            }
            _ => panic!("Expected UpdateAffiliations job"),
        }
    }
}

#[tokio::test]
async fn test_get_all_of_type_mixed_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push various job types
    queue
        .push(WorkerJob::UpdateCharacterInfo { character_id: 100 })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateAllianceInfo { alliance_id: 200 })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateCharacterInfo { character_id: 300 })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateCorporationInfo {
            corporation_id: 400,
        })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateCharacterInfo { character_id: 500 })
        .await
        .expect("Failed to push");

    // Query for character jobs only
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateCharacterInfo { character_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 3, "Expected 3 character jobs");

    // Verify only character jobs are returned
    for qj in &jobs {
        match qj.job {
            WorkerJob::UpdateCharacterInfo { .. } => {}
            _ => panic!("Expected only UpdateCharacterInfo jobs"),
        }
    }
}

#[tokio::test]
async fn test_get_all_of_type_scheduled_times() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    let now = Utc::now();
    let future_time = now + Duration::minutes(10);

    // Push job with immediate execution
    queue
        .push(WorkerJob::UpdateAllianceInfo { alliance_id: 100 })
        .await
        .expect("Failed to push");

    // Schedule job for future
    queue
        .schedule(
            WorkerJob::UpdateAllianceInfo { alliance_id: 200 },
            future_time,
        )
        .await
        .expect("Failed to schedule");

    // Query for alliance jobs
    let jobs = queue
        .get_all_of_type(WorkerJob::UpdateAllianceInfo { alliance_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(jobs.len(), 2, "Expected 2 alliance jobs");

    // Find the jobs by ID and verify their scheduled times
    let job_100 = jobs
        .iter()
        .find(|j| match j.job {
            WorkerJob::UpdateAllianceInfo { alliance_id } => alliance_id == 100,
            _ => false,
        })
        .expect("Job 100 not found");

    let job_200 = jobs
        .iter()
        .find(|j| match j.job {
            WorkerJob::UpdateAllianceInfo { alliance_id } => alliance_id == 200,
            _ => false,
        })
        .expect("Job 200 not found");

    // Job 100 should be scheduled around now (within 2 seconds)
    let diff_100 = (job_100.scheduled_at - now).num_seconds().abs();
    assert!(
        diff_100 < 2,
        "Job 100 should be scheduled close to now, diff: {} seconds",
        diff_100
    );

    // Job 200 should be scheduled around future_time (within 2 seconds)
    let diff_200 = (job_200.scheduled_at - future_time).num_seconds().abs();
    assert!(
        diff_200 < 2,
        "Job 200 should be scheduled close to future_time, diff: {} seconds",
        diff_200
    );
}

#[tokio::test]
async fn test_get_all_of_type_does_not_remove_jobs() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push multiple alliance jobs
    let alliance_ids = vec![1000, 2000, 3000];
    for id in &alliance_ids {
        let job = WorkerJob::UpdateAllianceInfo { alliance_id: *id };
        queue.push(job).await.expect("Failed to push job");
    }

    // Query for alliance jobs (should not remove them)
    let jobs_first = queue
        .get_all_of_type(WorkerJob::UpdateAllianceInfo { alliance_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(
        jobs_first.len(),
        3,
        "Expected 3 alliance jobs on first call"
    );

    // Query again - should still return all jobs
    let jobs_second = queue
        .get_all_of_type(WorkerJob::UpdateAllianceInfo { alliance_id: 0 })
        .await
        .expect("Failed to get jobs");

    assert_eq!(
        jobs_second.len(),
        3,
        "Expected 3 alliance jobs on second call"
    );

    // Verify IDs are the same
    let mut ids_first: Vec<i64> = jobs_first
        .iter()
        .map(|qj| match qj.job {
            WorkerJob::UpdateAllianceInfo { alliance_id } => alliance_id,
            _ => panic!("Expected UpdateAllianceInfo job"),
        })
        .collect();
    ids_first.sort();

    let mut ids_second: Vec<i64> = jobs_second
        .iter()
        .map(|qj| match qj.job {
            WorkerJob::UpdateAllianceInfo { alliance_id } => alliance_id,
            _ => panic!("Expected UpdateAllianceInfo job"),
        })
        .collect();
    ids_second.sort();

    assert_eq!(ids_first, ids_second, "IDs should be identical");
    assert_eq!(ids_first, alliance_ids, "IDs should match original");
}

#[tokio::test]
async fn test_get_all_of_type_no_cross_contamination() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = WorkerJobQueue::with_queue_name(redis.redis_pool.clone(), redis.queue_name());

    // Push jobs of all types
    queue
        .push(WorkerJob::UpdateCharacterInfo { character_id: 100 })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateAllianceInfo { alliance_id: 200 })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateCorporationInfo {
            corporation_id: 300,
        })
        .await
        .expect("Failed to push");
    queue
        .push(WorkerJob::UpdateAffiliations {
            character_ids: vec![1, 2, 3],
        })
        .await
        .expect("Failed to push");

    // Query each type and verify only correct type is returned
    let character_jobs = queue
        .get_all_of_type(WorkerJob::UpdateCharacterInfo { character_id: 0 })
        .await
        .expect("Failed to get character jobs");
    assert_eq!(character_jobs.len(), 1);
    assert!(matches!(
        character_jobs[0].job,
        WorkerJob::UpdateCharacterInfo { .. }
    ));

    let alliance_jobs = queue
        .get_all_of_type(WorkerJob::UpdateAllianceInfo { alliance_id: 0 })
        .await
        .expect("Failed to get alliance jobs");
    assert_eq!(alliance_jobs.len(), 1);
    assert!(matches!(
        alliance_jobs[0].job,
        WorkerJob::UpdateAllianceInfo { .. }
    ));

    let corporation_jobs = queue
        .get_all_of_type(WorkerJob::UpdateCorporationInfo { corporation_id: 0 })
        .await
        .expect("Failed to get corporation jobs");
    assert_eq!(corporation_jobs.len(), 1);
    assert!(matches!(
        corporation_jobs[0].job,
        WorkerJob::UpdateCorporationInfo { .. }
    ));

    let affiliation_jobs = queue
        .get_all_of_type(WorkerJob::UpdateAffiliations {
            character_ids: Vec::new(),
        })
        .await
        .expect("Failed to get affiliation jobs");
    assert_eq!(affiliation_jobs.len(), 1);
    assert!(matches!(
        affiliation_jobs[0].job,
        WorkerJob::UpdateAffiliations { .. }
    ));
}

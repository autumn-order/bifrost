//! Tests for WorkerQueue retry metadata functionality.
//!
//! This module verifies the behavior of scheduling jobs with retry metadata, ensuring that:
//! - Retry metadata is stored and retrieved correctly
//! - Deduplication works properly (fresh job vs retrying job are still duplicates)
//! - Retry metadata is preserved across schedule operations
//! - Cleanup removes orphaned retry metadata
//! - Multiple jobs can have independent retry metadata

use bifrost::server::model::worker::{RetryMetadata, WorkerJob};
use chrono::{Duration, Utc};
use fred::interfaces::HashesInterface;

use crate::util::redis::RedisTest;

use super::setup_test_queue;

mod schedule_retry {
    use super::*;

    /// Tests scheduling a job with retry metadata.
    ///
    /// Verifies that a job scheduled with retry metadata is added to the queue
    /// and the metadata is stored separately in the retry hash.
    ///
    /// Expected: Job added (true), retry metadata stored in hash
    #[tokio::test]
    async fn schedules_job_with_retry_metadata() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };
        let schedule_time = Utc::now() + Duration::minutes(5);
        let retry_metadata = RetryMetadata {
            attempt_count: 3,
            first_failed_at: Utc::now() - Duration::hours(1),
        };

        let result = queue
            .schedule(job.clone(), schedule_time, Some(retry_metadata.clone()))
            .await;
        assert!(result.is_ok(), "Schedule should succeed");
        assert_eq!(result.unwrap(), true, "Job should be added");

        // Verify retry metadata is stored in hash
        let retry_hash_key = format!("{}:retry", redis.queue_name());
        let job_key = serde_json::to_string(&job).unwrap();
        let stored_metadata: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &job_key)
            .await
            .expect("Failed to get retry metadata");

        assert!(stored_metadata.is_some(), "Retry metadata should be stored");

        let parsed_metadata: RetryMetadata =
            serde_json::from_str(&stored_metadata.unwrap()).unwrap();
        assert_eq!(
            parsed_metadata.attempt_count, retry_metadata.attempt_count,
            "Attempt count should match"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that fresh job and retrying job are still considered duplicates.
    ///
    /// Verifies that retry metadata does not affect deduplication - a fresh job
    /// scheduled after a retrying job should be detected as a duplicate.
    ///
    /// Expected: First schedule (with retry) succeeds, second schedule (fresh) returns false
    #[tokio::test]
    async fn retry_metadata_does_not_affect_deduplication() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };
        let schedule_time = Utc::now() + Duration::minutes(5);
        let retry_metadata = RetryMetadata {
            attempt_count: 2,
            first_failed_at: Utc::now() - Duration::minutes(30),
        };

        // Schedule with retry metadata
        let result1 = queue
            .schedule(job.clone(), schedule_time, Some(retry_metadata))
            .await;
        assert!(result1.is_ok(), "First schedule should succeed");
        assert_eq!(result1.unwrap(), true, "First job should be added");

        // Try to schedule same job without retry metadata
        let result2 = queue.schedule(job.clone(), schedule_time, None).await;
        assert!(
            result2.is_ok(),
            "Duplicate schedule should succeed (but not add)"
        );
        assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that fresh job scheduled first prevents retrying job.
    ///
    /// Verifies deduplication in reverse order - if a fresh job is scheduled first,
    /// attempting to schedule the same job with retry metadata should be detected
    /// as a duplicate.
    ///
    /// Expected: First schedule (fresh) succeeds, second schedule (with retry) returns false
    #[tokio::test]
    async fn fresh_job_prevents_retry_job_duplicate() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        };
        let schedule_time = Utc::now() + Duration::minutes(10);

        // Schedule without retry metadata first
        let result1 = queue.schedule(job.clone(), schedule_time, None).await;
        assert!(result1.is_ok(), "First schedule should succeed");
        assert_eq!(result1.unwrap(), true, "First job should be added");

        // Try to schedule same job with retry metadata
        let retry_metadata = RetryMetadata {
            attempt_count: 5,
            first_failed_at: Utc::now() - Duration::hours(2),
        };
        let result2 = queue
            .schedule(job.clone(), schedule_time, Some(retry_metadata))
            .await;
        assert!(
            result2.is_ok(),
            "Duplicate schedule should succeed (but not add)"
        );
        assert_eq!(result2.unwrap(), false, "Duplicate job should not be added");

        // Verify no retry metadata was stored (since job was duplicate)
        let retry_hash_key = format!("{}:retry", redis.queue_name());
        let job_key = serde_json::to_string(&job).unwrap();
        let stored_metadata: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &job_key)
            .await
            .expect("Failed to check retry metadata");

        assert!(
            stored_metadata.is_none(),
            "No retry metadata should be stored for duplicate"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests popping a job with retry metadata.
    ///
    /// Verifies that when a job with retry metadata is popped, the metadata
    /// is retrieved and the entry is removed from the retry hash.
    ///
    /// Expected: Job popped with correct retry metadata, hash entry removed
    #[tokio::test]
    async fn pop_retrieves_and_removes_retry_metadata() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateCorporationInfo {
            corporation_id: 98000001,
        };
        let schedule_time = Utc::now();
        let retry_metadata = RetryMetadata {
            attempt_count: 4,
            first_failed_at: Utc::now() - Duration::hours(1),
        };

        // Schedule job with retry metadata
        queue
            .schedule(job.clone(), schedule_time, Some(retry_metadata.clone()))
            .await
            .expect("Failed to schedule job");

        // Pop the job
        let popped = queue.pop().await.expect("Failed to pop job");
        assert!(popped.is_some(), "Job should be popped");

        let scheduled_job = popped.unwrap();
        assert_eq!(scheduled_job.job, job, "Job should match");
        assert!(
            scheduled_job.retry_metadata.is_some(),
            "Retry metadata should be present"
        );

        let popped_metadata = scheduled_job.retry_metadata.unwrap();
        assert_eq!(
            popped_metadata.attempt_count, retry_metadata.attempt_count,
            "Attempt count should match"
        );

        // Verify retry metadata was removed from hash
        let retry_hash_key = format!("{}:retry", redis.queue_name());
        let job_key = serde_json::to_string(&job).unwrap();
        let remaining_metadata: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &job_key)
            .await
            .expect("Failed to check retry metadata");

        assert!(
            remaining_metadata.is_none(),
            "Retry metadata should be removed after pop"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests popping a job without retry metadata.
    ///
    /// Verifies that when a job scheduled without retry metadata is popped,
    /// the retry_metadata field is None.
    ///
    /// Expected: Job popped successfully with retry_metadata = None
    #[tokio::test]
    async fn pop_job_without_retry_metadata() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateFactionInfo;
        let schedule_time = Utc::now();

        // Schedule job without retry metadata
        queue
            .schedule(job.clone(), schedule_time, None)
            .await
            .expect("Failed to schedule job");

        // Pop the job
        let popped = queue.pop().await.expect("Failed to pop job");
        assert!(popped.is_some(), "Job should be popped");

        let scheduled_job = popped.unwrap();
        assert_eq!(scheduled_job.job, job, "Job should match");
        assert!(
            scheduled_job.retry_metadata.is_none(),
            "Retry metadata should be None"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that multiple jobs can have independent retry metadata.
    ///
    /// Verifies that different jobs can be scheduled with different retry metadata
    /// and that the metadata is correctly associated with each job.
    ///
    /// Expected: Each job has its own independent retry metadata
    #[tokio::test]
    async fn multiple_jobs_have_independent_retry_metadata() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job1 = WorkerJob::UpdateCharacterInfo {
            character_id: 11111,
        };
        let job2 = WorkerJob::UpdateCharacterInfo {
            character_id: 22222,
        };
        let job3 = WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        };

        let schedule_time = Utc::now();

        let metadata1 = RetryMetadata {
            attempt_count: 1,
            first_failed_at: Utc::now() - Duration::minutes(15),
        };
        let metadata2 = RetryMetadata {
            attempt_count: 5,
            first_failed_at: Utc::now() - Duration::hours(2),
        };
        let metadata3 = RetryMetadata {
            attempt_count: 8,
            first_failed_at: Utc::now() - Duration::hours(3),
        };

        // Schedule all three jobs with different retry metadata
        queue
            .schedule(job1.clone(), schedule_time, Some(metadata1.clone()))
            .await
            .expect("Failed to schedule job1");
        queue
            .schedule(job2.clone(), schedule_time, Some(metadata2.clone()))
            .await
            .expect("Failed to schedule job2");
        queue
            .schedule(job3.clone(), schedule_time, Some(metadata3.clone()))
            .await
            .expect("Failed to schedule job3");

        // Verify all retry metadata is stored correctly
        let retry_hash_key = format!("{}:retry", redis.queue_name());

        let job1_key = serde_json::to_string(&job1).unwrap();
        let stored_metadata1: String = redis
            .redis_pool
            .hget(&retry_hash_key, &job1_key)
            .await
            .expect("Failed to get job1 metadata");
        let parsed_metadata1: RetryMetadata = serde_json::from_str(&stored_metadata1).unwrap();
        assert_eq!(parsed_metadata1.attempt_count, 1);

        let job2_key = serde_json::to_string(&job2).unwrap();
        let stored_metadata2: String = redis
            .redis_pool
            .hget(&retry_hash_key, &job2_key)
            .await
            .expect("Failed to get job2 metadata");
        let parsed_metadata2: RetryMetadata = serde_json::from_str(&stored_metadata2).unwrap();
        assert_eq!(parsed_metadata2.attempt_count, 5);

        let job3_key = serde_json::to_string(&job3).unwrap();
        let stored_metadata3: String = redis
            .redis_pool
            .hget(&retry_hash_key, &job3_key)
            .await
            .expect("Failed to get job3 metadata");
        let parsed_metadata3: RetryMetadata = serde_json::from_str(&stored_metadata3).unwrap();
        assert_eq!(parsed_metadata3.attempt_count, 8);

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that cleanup removes orphaned retry metadata.
    ///
    /// Verifies that when a job is removed from the queue (e.g., via stale cleanup),
    /// its associated retry metadata is also cleaned up to prevent memory leaks.
    ///
    /// Expected: Orphaned retry metadata is removed during cleanup
    #[tokio::test]
    async fn cleanup_removes_orphaned_retry_metadata() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 99999,
        };
        let schedule_time = Utc::now() - Duration::hours(2); // Schedule in past for cleanup
        let retry_metadata = RetryMetadata {
            attempt_count: 7,
            first_failed_at: Utc::now() - Duration::hours(3),
        };

        // Schedule job with retry metadata
        queue
            .schedule(job.clone(), schedule_time, Some(retry_metadata))
            .await
            .expect("Failed to schedule job");

        // Verify retry metadata exists
        let retry_hash_key = format!("{}:retry", redis.queue_name());
        let job_key = serde_json::to_string(&job).unwrap();
        let metadata_before: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &job_key)
            .await
            .expect("Failed to check metadata before cleanup");
        assert!(
            metadata_before.is_some(),
            "Retry metadata should exist before cleanup"
        );

        // Run cleanup to remove stale jobs
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");
        assert_eq!(removed, 1, "One stale job should be removed");

        // Verify retry metadata was also removed
        let metadata_after: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &job_key)
            .await
            .expect("Failed to check metadata after cleanup");
        assert!(
            metadata_after.is_none(),
            "Orphaned retry metadata should be removed"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that cleanup preserves retry metadata for non-stale jobs.
    ///
    /// Verifies that cleanup only removes retry metadata for jobs that are actually
    /// removed from the queue, and preserves metadata for jobs still in queue.
    ///
    /// Expected: Retry metadata for active jobs is preserved
    #[tokio::test]
    async fn cleanup_preserves_retry_metadata_for_active_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Schedule a stale job
        let stale_job = WorkerJob::UpdateCharacterInfo {
            character_id: 11111,
        };
        let stale_time = Utc::now() - Duration::hours(2);
        let stale_metadata = RetryMetadata {
            attempt_count: 3,
            first_failed_at: Utc::now() - Duration::hours(3),
        };
        queue
            .schedule(stale_job.clone(), stale_time, Some(stale_metadata))
            .await
            .expect("Failed to schedule stale job");

        // Schedule an active job
        let active_job = WorkerJob::UpdateCharacterInfo {
            character_id: 22222,
        };
        let active_time = Utc::now() + Duration::minutes(10);
        let active_metadata = RetryMetadata {
            attempt_count: 5,
            first_failed_at: Utc::now() - Duration::hours(1),
        };
        queue
            .schedule(
                active_job.clone(),
                active_time,
                Some(active_metadata.clone()),
            )
            .await
            .expect("Failed to schedule active job");

        // Run cleanup
        let removed = queue.cleanup_stale_jobs().await.expect("Failed to cleanup");
        assert_eq!(removed, 1, "One stale job should be removed");

        // Verify stale job's retry metadata was removed
        let retry_hash_key = format!("{}:retry", redis.queue_name());
        let stale_key = serde_json::to_string(&stale_job).unwrap();
        let stale_metadata_after: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &stale_key)
            .await
            .expect("Failed to check stale metadata");
        assert!(
            stale_metadata_after.is_none(),
            "Stale job's retry metadata should be removed"
        );

        // Verify active job's retry metadata is preserved
        let active_key = serde_json::to_string(&active_job).unwrap();
        let active_metadata_after: Option<String> = redis
            .redis_pool
            .hget(&retry_hash_key, &active_key)
            .await
            .expect("Failed to check active metadata");
        assert!(
            active_metadata_after.is_some(),
            "Active job's retry metadata should be preserved"
        );

        let parsed_metadata: RetryMetadata =
            serde_json::from_str(&active_metadata_after.unwrap()).unwrap();
        assert_eq!(
            parsed_metadata.attempt_count, active_metadata.attempt_count,
            "Active job's attempt count should match"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that retry metadata survives job rescheduling.
    ///
    /// Verifies that if a job with retry metadata is popped and rescheduled,
    /// the retry metadata can be updated and stored again.
    ///
    /// Expected: Updated retry metadata is stored correctly
    #[tokio::test]
    async fn retry_metadata_can_be_updated_on_reschedule() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        };

        // Initial schedule with retry metadata
        let initial_metadata = RetryMetadata {
            attempt_count: 2,
            first_failed_at: Utc::now() - Duration::minutes(30),
        };
        queue
            .schedule(job.clone(), Utc::now(), Some(initial_metadata.clone()))
            .await
            .expect("Failed to initial schedule");

        // Pop the job
        let popped = queue.pop().await.expect("Failed to pop job");
        assert!(popped.is_some());
        let scheduled_job = popped.unwrap();

        // Update retry metadata
        let mut updated_metadata = scheduled_job.retry_metadata.unwrap();
        updated_metadata.increment();

        // Reschedule with updated metadata
        let reschedule_time = Utc::now() + Duration::minutes(5);
        let result = queue
            .schedule(job.clone(), reschedule_time, Some(updated_metadata.clone()))
            .await;
        assert!(result.is_ok(), "Reschedule should succeed");
        assert_eq!(result.unwrap(), true, "Job should be added again");

        // Verify updated metadata is stored
        let retry_hash_key = format!("{}:retry", redis.queue_name());
        let job_key = serde_json::to_string(&job).unwrap();
        let stored_metadata: String = redis
            .redis_pool
            .hget(&retry_hash_key, &job_key)
            .await
            .expect("Failed to get updated metadata");

        let parsed_metadata: RetryMetadata = serde_json::from_str(&stored_metadata).unwrap();
        assert_eq!(
            parsed_metadata.attempt_count, 3,
            "Attempt count should be incremented"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests scheduling affiliation jobs with retry metadata.
    ///
    /// Verifies that batch jobs (like UpdateAffiliations) work correctly with
    /// retry metadata, including proper deduplication based on character IDs.
    ///
    /// Expected: Affiliation jobs with retry metadata work correctly
    #[tokio::test]
    async fn affiliation_jobs_work_with_retry_metadata() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateAffiliations {
            character_ids: vec![1001, 1002, 1003, 1004, 1005],
        };
        let schedule_time = Utc::now(); // Schedule immediately so we can pop it
        let retry_metadata = RetryMetadata {
            attempt_count: 2,
            first_failed_at: Utc::now() - Duration::minutes(20),
        };

        // Schedule affiliation job with retry metadata
        let result = queue
            .schedule(job.clone(), schedule_time, Some(retry_metadata.clone()))
            .await;
        assert!(result.is_ok(), "Schedule should succeed");
        assert_eq!(result.unwrap(), true, "Job should be added");

        // Try to schedule duplicate (same character IDs)
        let duplicate_result = queue.schedule(job.clone(), schedule_time, None).await;
        assert_eq!(
            duplicate_result.unwrap(),
            false,
            "Duplicate affiliation job should not be added"
        );

        // Pop and verify
        let popped = queue.pop().await.expect("Failed to pop job");
        assert!(popped.is_some());
        let scheduled_job = popped.unwrap();

        match scheduled_job.job {
            WorkerJob::UpdateAffiliations { character_ids } => {
                assert_eq!(character_ids.len(), 5, "Should have 5 character IDs");
            }
            _ => panic!("Wrong job type popped"),
        }

        assert!(
            scheduled_job.retry_metadata.is_some(),
            "Retry metadata should be present"
        );
        assert_eq!(
            scheduled_job.retry_metadata.unwrap().attempt_count,
            2,
            "Attempt count should match"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }
}

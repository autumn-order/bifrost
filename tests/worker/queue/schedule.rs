//! Tests for WorkerQueue::schedule method.
//!
//! This module verifies the behavior of the schedule operation for adding jobs with specific
//! execution times to the worker queue. Tests cover scheduling at future times, duplicate
//! detection across different job types, timestamp verification, and interaction with push.

use bifrost::server::model::worker::WorkerJob;
use chrono::{Duration, Utc};
use fred::interfaces::SortedSetsInterface;

use crate::util::redis::RedisTest;

use super::setup_test_queue;

static MAX_AFFILIATION_BATCH_SIZE: i64 = 1000;

mod schedule {
    use super::*;

    /// Tests successful scheduling of a new character job.
    ///
    /// Verifies that a character update job can be scheduled for future execution
    /// and returns true to indicate the job was added to the queue.
    ///
    /// Expected: Ok(true)
    #[tokio::test]
    async fn schedules_new_character_job() {
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

    /// Tests duplicate detection for character jobs.
    ///
    /// Verifies that scheduling the same character job twice, even at different times,
    /// results in the second schedule returning false to indicate duplicate detection.
    ///
    /// Expected: First schedule returns true, second schedule returns false
    #[tokio::test]
    async fn detects_duplicate_character_job() {
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

    /// Tests successful scheduling of a new alliance job.
    ///
    /// Verifies that an alliance update job can be scheduled for future execution
    /// and returns true to indicate the job was added to the queue.
    ///
    /// Expected: Ok(true)
    #[tokio::test]
    async fn schedules_new_alliance_job() {
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

    /// Tests duplicate detection for alliance jobs.
    ///
    /// Verifies that scheduling the same alliance job twice results in the second
    /// schedule returning false, indicating duplicate detection prevented re-adding.
    ///
    /// Expected: First schedule returns true, second schedule returns false
    #[tokio::test]
    async fn detects_duplicate_alliance_job() {
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

    /// Tests successful scheduling of a new corporation job.
    ///
    /// Verifies that a corporation update job can be scheduled for future execution
    /// and returns true to indicate the job was added to the queue.
    ///
    /// Expected: Ok(true)
    #[tokio::test]
    async fn schedules_new_corporation_job() {
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

    /// Tests duplicate detection for corporation jobs.
    ///
    /// Verifies that scheduling the same corporation job twice results in the second
    /// schedule returning false, indicating duplicate detection prevented re-adding.
    ///
    /// Expected: First schedule returns true, second schedule returns false
    #[tokio::test]
    async fn detects_duplicate_corporation_job() {
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

    /// Tests successful scheduling of an affiliation job.
    ///
    /// Verifies that an affiliation update job with multiple character IDs can be
    /// scheduled for future execution and returns true to indicate the job was added.
    ///
    /// Expected: Ok(true)
    #[tokio::test]
    async fn schedules_affiliation_job() {
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

    /// Tests duplicate detection for affiliation jobs with identical IDs.
    ///
    /// Verifies that scheduling the same affiliation job (same character IDs) twice
    /// results in the second schedule returning false, indicating duplicate detection.
    ///
    /// Expected: First schedule returns true, second schedule returns false
    #[tokio::test]
    async fn detects_duplicate_affiliation_job_same_ids() {
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

    /// Tests that affiliation jobs with different IDs are not duplicates.
    ///
    /// Verifies that scheduling two affiliation jobs with different sets of character
    /// IDs results in both jobs being added, as they are not considered duplicates.
    ///
    /// Expected: Both schedules return true
    #[tokio::test]
    async fn different_affiliation_ids_not_duplicate() {
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

    /// Tests that affiliation jobs at maximum batch size succeed.
    ///
    /// Verifies that an affiliation job with exactly MAX_AFFILIATION_BATCH_SIZE
    /// character IDs can be scheduled successfully without errors.
    ///
    /// Expected: Ok(true)
    #[tokio::test]
    async fn handles_max_affiliation_batch_size() {
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

    /// Tests scheduling multiple different job types.
    ///
    /// Verifies that jobs of different types (Character, Alliance, Corporation,
    /// Affiliation) can all be scheduled to the same queue successfully.
    ///
    /// Expected: All four schedules return true
    #[tokio::test]
    async fn handles_multiple_job_types() {
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

    /// Tests that same ID across different job types are not duplicates.
    ///
    /// Verifies that the same numeric ID used in different job types (Character,
    /// Alliance, Corporation) does not trigger duplicate detection, as each job
    /// type is distinct.
    ///
    /// Expected: All three schedules return true
    #[tokio::test]
    async fn same_id_different_types_not_duplicate() {
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

    /// Tests scheduling multiple jobs at different times.
    ///
    /// Verifies that multiple unique jobs can be scheduled at staggered future
    /// times without errors, ensuring the queue can handle temporal distribution.
    ///
    /// Expected: All 10 schedules return true
    #[tokio::test]
    async fn handles_multiple_jobs_at_different_times() {
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

    /// Tests that scheduled jobs are stored with correct timestamps.
    ///
    /// Verifies that when a job is scheduled, it is stored in Redis with the exact
    /// scheduled timestamp, ensuring proper temporal execution control.
    ///
    /// Expected: Job timestamp in Redis matches the scheduled time
    #[tokio::test]
    async fn stores_job_with_correct_timestamp() {
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
        let serialized = serde_json::to_string(&job).expect("Should serialize job");
        let score: Option<f64> = redis
            .redis_pool
            .zscore(&redis.queue_name(), &serialized)
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

    /// Tests that scheduling jobs in the past is allowed.
    ///
    /// Verifies that a job can be scheduled for a time in the past, which is useful
    /// for handling overflowed jobs or rescheduling missed executions.
    ///
    /// Expected: Ok(true) for past-scheduled job
    #[tokio::test]
    async fn allows_scheduling_in_past() {
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

    /// Tests duplicate detection between schedule and push.
    ///
    /// Verifies that a job pushed immediately is detected as a duplicate when
    /// attempting to schedule the same job, ensuring consistency across operations.
    ///
    /// Expected: Push returns true, schedule returns false
    #[tokio::test]
    async fn detects_duplicate_from_push() {
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

    /// Tests duplicate detection between push and schedule.
    ///
    /// Verifies that a job scheduled for future execution is detected as a duplicate
    /// when attempting to push the same job, ensuring consistency across operations.
    ///
    /// Expected: Schedule returns true, push returns false
    #[tokio::test]
    async fn detects_duplicate_from_schedule() {
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
}

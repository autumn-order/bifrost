//! Tests for WorkerQueue::len method.
//!
//! This module verifies the behavior of the len operation for counting jobs in the worker
//! queue. Tests cover empty queues, counting after push/pop operations, duplicate handling,
//! multiple job types, and consistency with scheduled jobs.

use bifrost::server::model::worker::WorkerJob;

use crate::util::redis::RedisTest;

use super::setup_test_queue;

mod len {
    use super::*;

    /// Tests that an empty queue reports length 0.
    ///
    /// Verifies that a newly created queue with no jobs returns a length of 0,
    /// establishing the baseline behavior for an empty queue.
    ///
    /// Expected: len() returns 0
    #[tokio::test]
    async fn returns_zero_for_empty_queue() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(len, 0, "Empty queue should have length 0");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that len correctly counts multiple pushed jobs.
    ///
    /// Verifies that after pushing 5 distinct jobs to the queue, the length
    /// accurately reflects the total number of jobs added.
    ///
    /// Expected: len() returns 5 after pushing 5 jobs
    #[tokio::test]
    async fn counts_multiple_pushed_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Push 5 different jobs
        for i in 1..=5 {
            let job = WorkerJob::UpdateCharacterInfo { character_id: i };
            queue.push(job).await.expect("Should push job");
        }

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(len, 5, "Queue with 5 jobs should have length 5");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that len does not count duplicate job attempts.
    ///
    /// Verifies that when the same job is pushed multiple times, the queue's
    /// duplicate detection prevents adding the same job twice, and len only
    /// counts the unique job once.
    ///
    /// Expected: len() returns 1 after pushing the same job 3 times
    #[tokio::test]
    async fn does_not_count_duplicates() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };

        // Push same job 3 times
        queue.push(job.clone()).await.expect("Should push job");
        queue.push(job.clone()).await.expect("Should push job");
        queue.push(job.clone()).await.expect("Should push job");

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(
            len, 1,
            "Queue should have length 1 since duplicates are not added"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that len correctly decreases after popping a job.
    ///
    /// Verifies that after pushing 3 jobs and popping 1 job, the queue length
    /// accurately reflects the remaining number of jobs (2).
    ///
    /// Expected: len() returns 2 after pushing 3 jobs and popping 1
    #[tokio::test]
    async fn decreases_after_pop() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Push 3 jobs
        for i in 1..=3 {
            let job = WorkerJob::UpdateCharacterInfo { character_id: i };
            queue.push(job).await.expect("Should push job");
        }

        // Pop one job
        queue.pop().await.expect("Should pop job");

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(len, 2, "Queue should have length 2 after popping 1 job");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that len correctly counts jobs of different types.
    ///
    /// Verifies that the queue can hold and correctly count jobs of all available
    /// types (Character, Alliance, Corporation, Affiliation, and Faction updates).
    ///
    /// Expected: len() returns 5 after pushing one job of each type
    #[tokio::test]
    async fn counts_different_job_types() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        queue
            .push(WorkerJob::UpdateCharacterInfo {
                character_id: 12345,
            })
            .await
            .expect("Should push job");
        queue
            .push(WorkerJob::UpdateAllianceInfo {
                alliance_id: 99000001,
            })
            .await
            .expect("Should push job");
        queue
            .push(WorkerJob::UpdateCorporationInfo {
                corporation_id: 98000001,
            })
            .await
            .expect("Should push job");
        queue
            .push(WorkerJob::UpdateAffiliations {
                character_ids: vec![11111, 22222],
            })
            .await
            .expect("Should push job");
        queue
            .push(WorkerJob::UpdateFactionInfo {})
            .await
            .expect("Should push job");

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(
            len, 5,
            "Queue with 5 different job types should have length 5"
        );

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that len correctly tracks mixed push and pop operations.
    ///
    /// Verifies that the queue length is accurately maintained through a sequence
    /// of push and pop operations: pushing 5 jobs, popping 2, then pushing 2 more.
    ///
    /// Expected: len() returns 3 after popping, then 5 after adding more jobs
    #[tokio::test]
    async fn tracks_mixed_operations() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        // Push 5 jobs
        for i in 1..=5 {
            let job = WorkerJob::UpdateCharacterInfo { character_id: i };
            queue.push(job).await.expect("Should push job");
        }

        // Pop 2 jobs
        queue.pop().await.expect("Should pop job");
        queue.pop().await.expect("Should pop job");

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(len, 3, "Queue should have 3 jobs after popping 2");

        // Push 2 more jobs
        queue
            .push(WorkerJob::UpdateAllianceInfo { alliance_id: 1001 })
            .await
            .expect("Should push job");
        queue
            .push(WorkerJob::UpdateCorporationInfo {
                corporation_id: 2001,
            })
            .await
            .expect("Should push job");

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(len, 5, "Queue should have 5 jobs after adding 2 more");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }

    /// Tests that scheduled jobs are counted in queue length.
    ///
    /// Verifies that a job scheduled for future execution is immediately counted
    /// in the queue length, even though it's not yet due for processing.
    ///
    /// Expected: len() returns 1 after scheduling a job for the future
    #[tokio::test]
    async fn counts_scheduled_jobs() {
        let redis = RedisTest::new().await.expect("Failed to create Redis test");
        let queue = setup_test_queue(&redis);

        let job = WorkerJob::UpdateCharacterInfo {
            character_id: 12345,
        };
        let scheduled_at = chrono::Utc::now() + chrono::Duration::seconds(3600);

        queue
            .schedule(job, scheduled_at)
            .await
            .expect("Should schedule job");

        let len = queue.len().await.expect("Should get queue length");
        assert_eq!(len, 1, "Queue should have length 1 after scheduling a job");

        redis.cleanup().await.expect("Failed to cleanup Redis");
    }
}

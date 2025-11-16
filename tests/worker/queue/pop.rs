//! Tests for WorkerJobQueue::pop method
//!
//! These tests verify the pop method's behavior including:
//! - Popping from an empty queue
//! - Popping a single job
//! - Popping jobs in chronological order (FIFO)
//! - Popping different job types (Character, Alliance, Corporation)
//! - Verifying jobs are removed from the queue after popping
//! - Popping multiple jobs sequentially

use bifrost::server::model::worker::WorkerJob;
use chrono::{Duration, Utc};

use crate::util::redis::RedisTest;

use super::setup_test_queue;

#[tokio::test]
async fn test_pop_from_empty_queue() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let result = queue.pop().await;
    assert!(result.is_ok(), "Pop from empty queue should succeed");
    assert_eq!(result.unwrap(), None, "Should return None for empty queue");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_single_character_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);

    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Push job
    let push_result = queue.push(job.clone()).await;
    assert!(
        push_result.is_ok() && push_result.unwrap(),
        "Job should be pushed"
    );

    // Pop job
    let pop_result = queue.pop().await;
    assert!(pop_result.is_ok(), "Pop should succeed");

    let popped_job = pop_result.unwrap();
    assert!(popped_job.is_some(), "Should return a job");

    // Verify it's the same job
    match popped_job.unwrap() {
        WorkerJob::UpdateCharacterInfo { character_id } => {
            assert_eq!(character_id, 12345, "Should be the same character ID");
        }
        _ => panic!("Should be UpdateCharacterInfo job"),
    }

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_single_alliance_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };

    // Push job
    let push_result = queue.push(job.clone()).await;
    assert!(
        push_result.is_ok() && push_result.unwrap(),
        "Job should be pushed"
    );

    // Pop job
    let pop_result = queue.pop().await;
    assert!(pop_result.is_ok(), "Pop should succeed");

    let popped_job = pop_result.unwrap();
    assert!(popped_job.is_some(), "Should return a job");

    // Verify it's the same job
    match popped_job.unwrap() {
        WorkerJob::UpdateAllianceInfo { alliance_id } => {
            assert_eq!(alliance_id, 99000001, "Should be the same alliance ID");
        }
        _ => panic!("Should be UpdateAllianceInfo job"),
    }

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_single_corporation_job() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };

    // Push job
    let push_result = queue.push(job.clone()).await;
    assert!(
        push_result.is_ok() && push_result.unwrap(),
        "Job should be pushed"
    );

    // Pop job
    let pop_result = queue.pop().await;
    assert!(pop_result.is_ok(), "Pop should succeed");

    let popped_job = pop_result.unwrap();
    assert!(popped_job.is_some(), "Should return a job");

    // Verify it's the same job
    match popped_job.unwrap() {
        WorkerJob::UpdateCorporationInfo { corporation_id } => {
            assert_eq!(
                corporation_id, 98000001,
                "Should be the same corporation ID"
            );
        }
        _ => panic!("Should be UpdateCorporationInfo job"),
    }

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_removes_job_from_queue() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };

    // Push job
    queue.push(job.clone()).await.expect("Push should succeed");

    // Pop job
    let pop_result = queue.pop().await;
    assert!(pop_result.is_ok() && pop_result.unwrap().is_some());

    // Try to pop again - should be empty
    let second_pop = queue.pop().await;
    assert!(second_pop.is_ok(), "Second pop should succeed");
    assert_eq!(
        second_pop.unwrap(),
        None,
        "Queue should be empty after first pop"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_returns_jobs_in_chronological_order() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let now = Utc::now();

    // Schedule jobs at different times (all in the past so they're immediately available)
    let job1 = WorkerJob::UpdateCharacterInfo {
        character_id: 11111,
    };
    let job2 = WorkerJob::UpdateCharacterInfo {
        character_id: 22222,
    };
    let job3 = WorkerJob::UpdateCharacterInfo {
        character_id: 33333,
    };

    // Schedule job2 first (10 minutes ago - middle)
    queue
        .schedule(job2.clone(), now - Duration::minutes(10))
        .await
        .expect("Should schedule job2");

    // Schedule job3 second (5 minutes ago - newest)
    queue
        .schedule(job3.clone(), now - Duration::minutes(5))
        .await
        .expect("Should schedule job3");

    // Schedule job1 third (15 minutes ago - earliest)
    queue
        .schedule(job1.clone(), now - Duration::minutes(15))
        .await
        .expect("Should schedule job1");

    // Pop jobs - should come out in chronological order (job1, job2, job3)
    let popped1 = queue
        .pop()
        .await
        .expect("Should pop")
        .expect("Should have job");
    let popped2 = queue
        .pop()
        .await
        .expect("Should pop")
        .expect("Should have job");
    let popped3 = queue
        .pop()
        .await
        .expect("Should pop")
        .expect("Should have job");

    match popped1 {
        WorkerJob::UpdateCharacterInfo { character_id } => {
            assert_eq!(character_id, 11111, "First job should be 11111");
        }
        _ => panic!("Should be UpdateCharacterInfo"),
    }

    match popped2 {
        WorkerJob::UpdateCharacterInfo { character_id } => {
            assert_eq!(character_id, 22222, "Second job should be 22222");
        }
        _ => panic!("Should be UpdateCharacterInfo"),
    }

    match popped3 {
        WorkerJob::UpdateCharacterInfo { character_id } => {
            assert_eq!(character_id, 33333, "Third job should be 33333");
        }
        _ => panic!("Should be UpdateCharacterInfo"),
    }

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_multiple_jobs_sequentially() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    // Push 5 different jobs
    for i in 1..=5 {
        let job = WorkerJob::UpdateCharacterInfo { character_id: i };
        queue.push(job).await.expect("Push should succeed");
    }

    // Pop all 5 jobs
    for i in 1..=5 {
        let result = queue.pop().await;
        assert!(result.is_ok(), "Pop {} should succeed", i);
        assert!(result.unwrap().is_some(), "Should return job {}", i);
    }

    // Queue should now be empty
    let result = queue.pop().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None, "Queue should be empty");

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_mixed_job_types_in_order() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let now = Utc::now();

    // Schedule different job types at different times (all in the past so they're immediately available)
    let char_job = WorkerJob::UpdateCharacterInfo {
        character_id: 12345,
    };
    let alliance_job = WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    };
    let corp_job = WorkerJob::UpdateCorporationInfo {
        corporation_id: 98000001,
    };

    queue
        .schedule(alliance_job.clone(), now - Duration::minutes(1))
        .await
        .expect("Should schedule alliance job");

    queue
        .schedule(char_job.clone(), now - Duration::minutes(3))
        .await
        .expect("Should schedule char job");

    queue
        .schedule(corp_job.clone(), now - Duration::minutes(2))
        .await
        .expect("Should schedule corp job");

    // Pop in chronological order
    let pop1 = queue
        .pop()
        .await
        .expect("Should pop")
        .expect("Should have job");
    let pop2 = queue
        .pop()
        .await
        .expect("Should pop")
        .expect("Should have job");
    let pop3 = queue
        .pop()
        .await
        .expect("Should pop")
        .expect("Should have job");

    // Verify order: char, corp, alliance
    assert!(
        matches!(pop1, WorkerJob::UpdateCharacterInfo { .. }),
        "First should be character job"
    );
    assert!(
        matches!(pop2, WorkerJob::UpdateCorporationInfo { .. }),
        "Second should be corporation job"
    );
    assert!(
        matches!(pop3, WorkerJob::UpdateAllianceInfo { .. }),
        "Third should be alliance job"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_after_push_immediate_availability() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 99999,
    };

    // Push a job with current timestamp
    queue.push(job.clone()).await.expect("Push should succeed");

    // Immediately pop - should be available
    let result = queue.pop().await;
    assert!(result.is_ok(), "Pop should succeed immediately after push");
    assert!(
        result.unwrap().is_some(),
        "Job should be available immediately"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_future_scheduled_job_not_returned_until_due() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 55555,
    };

    // Schedule job 10 minutes in the future
    let future_time = Utc::now() + Duration::minutes(10);
    queue
        .schedule(job.clone(), future_time)
        .await
        .expect("Schedule should succeed");

    // Pop should NOT return the job because it's not due yet
    let result = queue.pop().await;
    assert!(result.is_ok(), "Pop should succeed");
    assert_eq!(
        result.unwrap(),
        None,
        "Should NOT return job scheduled in future"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

#[tokio::test]
async fn test_pop_past_scheduled_job_is_immediately_available() {
    let redis = RedisTest::new().await.expect("Failed to create Redis test");
    let queue = setup_test_queue(&redis);
    let job = WorkerJob::UpdateCharacterInfo {
        character_id: 77777,
    };

    // Schedule job in the past (should be immediately available)
    let past_time = Utc::now() - Duration::minutes(5);
    queue
        .schedule(job.clone(), past_time)
        .await
        .expect("Schedule should succeed");

    // Pop should return the job because it's already due
    let result = queue.pop().await;
    assert!(result.is_ok(), "Pop should succeed");
    assert!(
        result.unwrap().is_some(),
        "Should return job scheduled in past"
    );

    redis.cleanup().await.expect("Failed to cleanup Redis");
}

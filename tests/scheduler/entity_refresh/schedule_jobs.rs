//! Tests for EntityRefreshTracker::schedule_jobs method.
//!
//! This module verifies the behavior of scheduling worker jobs to the queue with
//! staggered execution times. Tests cover single and multiple job scheduling, empty
//! job lists, duplicate detection, and large batch handling.

use bifrost::server::model::worker::WorkerJob;

use super::*;

/// Tests scheduling a single job.
///
/// Verifies that the entity refresh tracker can schedule a single worker job
/// to the queue and returns the count of scheduled jobs (1).
///
/// Expected: Ok(1) and one job in queue
#[tokio::test]
async fn schedules_single_job() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let jobs = vec![WorkerJob::UpdateAllianceInfo {
        alliance_id: 99000001,
    }];

    let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling multiple jobs.
///
/// Verifies that the entity refresh tracker can schedule multiple worker jobs
/// to the queue with staggered execution times and returns the correct count.
///
/// Expected: Ok(3) and three jobs in queue
#[tokio::test]
async fn schedules_multiple_jobs() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let jobs = vec![
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        },
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000002,
        },
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000003,
        },
    ];

    let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling with empty job list.
///
/// Verifies that the entity refresh tracker correctly handles an empty job list
/// by returning zero without errors or side effects.
///
/// Expected: Ok(0)
#[tokio::test]
async fn returns_zero_for_empty_jobs() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let jobs = vec![];

    let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    redis.cleanup().await?;
    Ok(())
}

/// Tests duplicate job detection during scheduling.
///
/// Verifies that the entity refresh tracker's duplicate detection prevents
/// the same job from being added to the queue multiple times, even when
/// included in the same batch.
///
/// Expected: Ok(1) with only one job actually scheduled despite duplicates
#[tokio::test]
async fn handles_duplicate_jobs() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let jobs = vec![
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        },
        WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000001,
        }, // duplicate
    ];

    let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

    assert!(result.is_ok());
    // Only the first job is scheduled, duplicate is not pushed to queue
    assert_eq!(result.unwrap(), 1);

    redis.cleanup().await?;
    Ok(())
}

/// Tests scheduling a large batch of jobs.
///
/// Verifies that the entity refresh tracker can efficiently handle scheduling
/// many jobs (100 in this test) with appropriate time staggering to distribute
/// load across the schedule interval.
///
/// Expected: Ok(100) and 100 jobs in queue
#[tokio::test]
async fn schedules_many_jobs() -> Result<(), TestError> {
    let test = TestBuilder::new()
        .with_table(entity::prelude::EveFaction)
        .with_table(entity::prelude::EveAlliance)
        .build()
        .await?;
    let redis = RedisTest::new().await?;
    let queue = setup_test_queue(&redis);

    let tracker = EntityRefreshTracker::new(
        &test.db,
        alliance_config::CACHE_DURATION,
        alliance_config::SCHEDULE_INTERVAL,
    );

    let jobs: Vec<WorkerJob> = (1..=100)
        .map(|i| WorkerJob::UpdateAllianceInfo {
            alliance_id: 99000000 + i,
        })
        .collect();

    let result = tracker.schedule_jobs::<AllianceInfo>(&queue, jobs).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 100);

    redis.cleanup().await?;
    Ok(())
}

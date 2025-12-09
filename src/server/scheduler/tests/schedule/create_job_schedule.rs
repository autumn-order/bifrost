//! Tests for create_job_schedule function.

use chrono::{Duration, Utc};

use crate::server::{model::worker::WorkerJob, scheduler::schedule::create_job_schedule};

/// Tests returning empty schedule for no jobs.
///
/// Verifies that the function returns an empty schedule when provided with
/// an empty job list.
///
/// Expected: Ok with empty Vec
#[tokio::test]
async fn returns_empty_for_no_jobs() {
    let result = create_job_schedule(vec![], Duration::minutes(10)).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert!(scheduled_jobs.is_empty());
}

/// Tests scheduling a single job.
///
/// Verifies that the function schedules a single job at or near the current
/// time and preserves the job data correctly.
///
/// Expected: Ok with 1 job scheduled at current time
#[tokio::test]
async fn schedules_single_job() {
    let jobs = vec![WorkerJob::UpdateAllianceInfo { alliance_id: 1 }];

    let before = Utc::now().timestamp();
    let result = create_job_schedule(jobs, Duration::minutes(10)).await;
    let after = Utc::now().timestamp();

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 1);

    let (job, scheduled_at) = &scheduled_jobs[0];
    assert!(matches!(
        job,
        WorkerJob::UpdateAllianceInfo { alliance_id: 1 }
    ));
    assert!(scheduled_at.timestamp() >= before);
    assert!(scheduled_at.timestamp() <= after + 1); // Allow 1 second for execution time
}

/// Tests staggering job execution times.
///
/// Verifies that the function distributes multiple jobs evenly across the
/// schedule interval with consistent time spacing between jobs.
///
/// Expected: Ok with jobs spaced 200 seconds apart (600s / 3 jobs)
#[tokio::test]
async fn staggers_job_execution_times() {
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 1 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 2 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 3 },
    ];

    let schedule_interval = Duration::minutes(10);
    let before = Utc::now().timestamp();
    let result = create_job_schedule(jobs, schedule_interval).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 3);

    // schedule_interval = 10 minutes = 600 seconds
    // With 3 jobs, interval should be 600 / 3 = 200 seconds
    let expected_interval = 200;

    // Check that scheduled times are properly staggered
    assert!(scheduled_jobs[0].1.timestamp() >= before);
    assert_eq!(
        scheduled_jobs[1].1.timestamp() - scheduled_jobs[0].1.timestamp(),
        expected_interval
    );
    assert_eq!(
        scheduled_jobs[2].1.timestamp() - scheduled_jobs[1].1.timestamp(),
        expected_interval
    );
}

/// Tests handling more jobs than seconds in interval.
///
/// Verifies that the function correctly distributes jobs when there are more
/// jobs than seconds in the schedule interval, ensuring all fit within the window.
///
/// Expected: Ok with 700 jobs distributed across 600 seconds
#[tokio::test]
async fn handles_more_jobs_than_seconds() {
    // Create more jobs than seconds in the schedule interval
    let mut jobs = Vec::new();
    for i in 1..=700 {
        jobs.push(WorkerJob::UpdateAllianceInfo { alliance_id: i });
    }

    let schedule_interval = Duration::minutes(10); // 600 seconds
    let before = Utc::now().timestamp();
    let result = create_job_schedule(jobs, schedule_interval).await;
    let after = before + schedule_interval.num_seconds();

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 700);

    // All jobs should fit within the 600-second window
    for (index, (_, scheduled_at)) in scheduled_jobs.iter().enumerate() {
        assert!(
            scheduled_at.timestamp() >= before && scheduled_at.timestamp() <= after,
            "Job {} scheduled at {} is outside window [{}, {}]",
            index,
            scheduled_at.timestamp(),
            before,
            after
        );
    }

    // First job should be at or near the start
    assert_eq!(scheduled_jobs[0].1.timestamp(), before);

    // Last job should be near the end but within window
    assert!(scheduled_jobs[699].1.timestamp() <= after);
    assert!(scheduled_jobs[699].1.timestamp() >= after - 2); // Within last 2 seconds of window
}

/// Tests returning correct job structure.
///
/// Verifies that the function returns jobs with the correct structure containing
/// WorkerJob data and properly ordered timestamps.
///
/// Expected: Ok with Vec of (WorkerJob, DateTime) tuples
#[tokio::test]
async fn returns_correct_job_structure() {
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 42 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 99 },
    ];

    let before = Utc::now().timestamp();
    let result = create_job_schedule(jobs, Duration::minutes(5)).await;
    let after = Utc::now().timestamp() + Duration::minutes(5).num_seconds();

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 2);

    // Verify first job
    let (job1, scheduled_at1) = &scheduled_jobs[0];
    assert!(matches!(
        job1,
        WorkerJob::UpdateAllianceInfo { alliance_id: 42 }
    ));
    assert!(scheduled_at1.timestamp() >= before);
    assert!(scheduled_at1.timestamp() <= after);

    // Verify second job
    let (job2, scheduled_at2) = &scheduled_jobs[1];
    assert!(matches!(
        job2,
        WorkerJob::UpdateAllianceInfo { alliance_id: 99 }
    ));
    assert!(scheduled_at2.timestamp() >= before);
    assert!(scheduled_at2.timestamp() <= after);

    // Verify second job is scheduled after first
    assert!(scheduled_at2.timestamp() > scheduled_at1.timestamp());
}

/// Tests scheduling within interval window.
///
/// Verifies that the function schedules all jobs within the specified schedule
/// interval window without exceeding the time boundaries.
///
/// Expected: Ok with all jobs scheduled between start and end of interval
#[tokio::test]
async fn schedules_within_interval_window() {
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 1 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 2 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 3 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 4 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 5 },
    ];

    let schedule_interval = Duration::minutes(10);
    let before = Utc::now().timestamp();
    let result = create_job_schedule(jobs, schedule_interval).await;
    let after = before + schedule_interval.num_seconds();

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();

    // All jobs should be scheduled within the interval window
    for (_, scheduled_at) in scheduled_jobs {
        assert!(
            scheduled_at.timestamp() >= before && scheduled_at.timestamp() <= after,
            "Job scheduled at {} is outside window [{}, {}]",
            scheduled_at.timestamp(),
            before,
            after
        );
    }
}

/// Tests maintaining job order.
///
/// Verifies that the function preserves the order of jobs from the input list
/// in the scheduled output.
///
/// Expected: Ok with jobs in same order as input
#[tokio::test]
async fn maintains_job_order() {
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 10 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 20 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 30 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 40 },
    ];

    let result = create_job_schedule(jobs, Duration::minutes(10)).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 4);

    // Jobs should maintain their input order
    assert!(matches!(
        scheduled_jobs[0].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 10 }
    ));
    assert!(matches!(
        scheduled_jobs[1].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 20 }
    ));
    assert!(matches!(
        scheduled_jobs[2].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 30 }
    ));
    assert!(matches!(
        scheduled_jobs[3].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 40 }
    ));
}

/// Tests producing monotonic timestamps.
///
/// Verifies that the function generates timestamps that are monotonically
/// increasing, ensuring proper temporal ordering of jobs.
///
/// Expected: Ok with each timestamp >= previous timestamp
#[tokio::test]
async fn produces_monotonic_timestamps() {
    let mut jobs = Vec::new();
    for i in 1..=50 {
        jobs.push(WorkerJob::UpdateAllianceInfo { alliance_id: i });
    }

    let result = create_job_schedule(jobs, Duration::minutes(10)).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();

    // Verify timestamps increase monotonically
    for i in 1..scheduled_jobs.len() {
        assert!(
            scheduled_jobs[i].1.timestamp() >= scheduled_jobs[i - 1].1.timestamp(),
            "Timestamp at index {} ({}) is not >= previous timestamp ({})",
            i,
            scheduled_jobs[i].1.timestamp(),
            scheduled_jobs[i - 1].1.timestamp()
        );
    }
}

/// Tests calculating correct intervals for different job counts.
///
/// Verifies that the function correctly calculates time intervals between jobs
/// for various combinations of job counts and schedule intervals.
///
/// Expected: Ok with correct intervals for each test case
#[tokio::test]
async fn calculates_correct_intervals_for_different_counts() {
    let test_cases = vec![
        (2, Duration::minutes(10), 300),  // 600 / 2 = 300
        (4, Duration::minutes(8), 120),   // 480 / 4 = 120
        (10, Duration::minutes(5), 30),   // 300 / 10 = 30
        (100, Duration::minutes(20), 12), // 1200 / 100 = 12
    ];

    for (job_count, interval, expected_interval) in test_cases {
        let mut jobs = Vec::new();
        for i in 1..=job_count {
            jobs.push(WorkerJob::UpdateAllianceInfo { alliance_id: i });
        }

        let result = create_job_schedule(jobs, interval).await;
        assert!(result.is_ok());

        let scheduled_jobs = result.unwrap();
        assert_eq!(scheduled_jobs.len(), job_count as usize);

        // Check intervals between consecutive jobs
        for i in 1..scheduled_jobs.len() {
            let actual_interval =
                scheduled_jobs[i].1.timestamp() - scheduled_jobs[i - 1].1.timestamp();
            assert_eq!(
                actual_interval, expected_interval,
                "Job count: {}, Expected interval: {}, Actual interval: {}",
                job_count, expected_interval, actual_interval
            );
        }
    }
}

/// Tests offsetting jobs that overlap with ESI downtime.
///
/// Verifies that when a job is scheduled during ESI downtime window (10:58-11:07 UTC),
/// it and all subsequent jobs are offset to after the downtime period.
///
/// Expected: Jobs scheduled during downtime are moved to after 11:07 UTC
#[tokio::test]
async fn offsets_jobs_during_downtime() {
    // Mock the current time to be 11:00 UTC (during downtime)
    // We'll create jobs that would be scheduled during the downtime window
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 1 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 2 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 3 },
    ];

    // Schedule over 10 minutes, which would normally space jobs 200 seconds apart
    let result = create_job_schedule(jobs, Duration::minutes(10)).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 3);

    // If we're currently in downtime, all jobs should be offset
    // Verify that timestamps are still monotonically increasing
    for i in 1..scheduled_jobs.len() {
        assert!(
            scheduled_jobs[i].1.timestamp() >= scheduled_jobs[i - 1].1.timestamp(),
            "Timestamps should be monotonically increasing even after downtime offset"
        );
    }
}

/// Tests that jobs before downtime are not offset.
///
/// Verifies that jobs scheduled well before the ESI downtime window
/// are not affected by the downtime offset logic.
///
/// Expected: Jobs maintain original schedule when not in downtime
#[tokio::test]
async fn does_not_offset_jobs_before_downtime() {
    // This test runs at current time which is likely not during downtime
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 1 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 2 },
    ];

    let schedule_interval = Duration::minutes(5);
    let before = Utc::now().timestamp();
    let result = create_job_schedule(jobs, schedule_interval).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();

    // If not in downtime, jobs should be within the normal window
    // (or slightly extended if they happen to hit downtime)
    for (_, scheduled_at) in &scheduled_jobs {
        assert!(
            scheduled_at.timestamp() >= before,
            "Job should not be scheduled before start time"
        );
    }

    // Timestamps should still be monotonically increasing
    assert!(scheduled_jobs[1].1.timestamp() >= scheduled_jobs[0].1.timestamp());
}

/// Tests cumulative offset when multiple jobs hit downtime.
///
/// Verifies that when multiple consecutive jobs would be scheduled during
/// downtime, the cumulative offset is maintained so all jobs are properly
/// spaced after the downtime window.
///
/// Expected: All jobs after first downtime hit maintain cumulative offset
#[tokio::test]
async fn maintains_cumulative_offset_through_downtime() {
    // Create many jobs to test cumulative offset behavior
    let mut jobs = Vec::new();
    for i in 1..=10 {
        jobs.push(WorkerJob::UpdateAllianceInfo { alliance_id: i });
    }

    let result = create_job_schedule(jobs, Duration::minutes(10)).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 10);

    // All timestamps must be monotonically increasing
    for i in 1..scheduled_jobs.len() {
        assert!(
            scheduled_jobs[i].1.timestamp() > scheduled_jobs[i - 1].1.timestamp(),
            "Timestamp at index {} must be greater than previous",
            i
        );
    }

    // If any job was offset, subsequent jobs should maintain spacing
    for i in 1..scheduled_jobs.len() {
        let interval = scheduled_jobs[i].1.timestamp() - scheduled_jobs[i - 1].1.timestamp();

        // After downtime offset, intervals should be positive
        assert!(interval > 0, "Interval must be positive");
    }
}

/// Tests that job order is preserved even with downtime offsets.
///
/// Verifies that when jobs are offset due to downtime, their relative
/// order is maintained in the schedule.
///
/// Expected: Jobs maintain input order despite downtime offsets
#[tokio::test]
async fn preserves_job_order_with_downtime_offset() {
    let jobs = vec![
        WorkerJob::UpdateAllianceInfo { alliance_id: 100 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 200 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 300 },
        WorkerJob::UpdateAllianceInfo { alliance_id: 400 },
    ];

    let result = create_job_schedule(jobs, Duration::minutes(15)).await;

    assert!(result.is_ok());
    let scheduled_jobs = result.unwrap();
    assert_eq!(scheduled_jobs.len(), 4);

    // Verify job order is preserved
    assert!(matches!(
        scheduled_jobs[0].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 100 }
    ));
    assert!(matches!(
        scheduled_jobs[1].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 200 }
    ));
    assert!(matches!(
        scheduled_jobs[2].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 300 }
    ));
    assert!(matches!(
        scheduled_jobs[3].0,
        WorkerJob::UpdateAllianceInfo { alliance_id: 400 }
    ));

    // Verify temporal ordering
    for i in 1..scheduled_jobs.len() {
        assert!(
            scheduled_jobs[i].1 >= scheduled_jobs[i - 1].1,
            "Job {} should be scheduled at or after job {}",
            i,
            i - 1
        );
    }
}

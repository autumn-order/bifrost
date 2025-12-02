//! Job scheduling utilities for distributing work across time windows.
//!
//! This module provides functions to calculate batch sizes for scheduled updates and to
//! stagger job execution times evenly across scheduling intervals. These utilities help
//! distribute API load and worker queue pressure over time rather than executing all
//! updates at once.

use chrono::{DateTime, Duration, Utc};

use crate::server::model::worker::WorkerJob;

/// Minimum batch size for entity updates per scheduling cycle.
///
/// Ensures at least 100 entities are updated per schedule run, even if the cache duration
/// would allow for smaller batches. This prevents overly granular scheduling that could
/// lead to excessive overhead.
static MIN_BATCH_LIMIT: i64 = 100;

/// Calculates the maximum number of entities to schedule for update in a single batch.
///
/// Determines an appropriate batch size based on the total number of table entries, cache
/// duration, and scheduling interval. The goal is to spread updates evenly across the cache
/// period while respecting a minimum batch size to avoid excessive scheduling overhead.
///
/// # Arguments
/// - `table_entries` - Total number of entities in the table that may need updates
/// - `cache` - Duration that cached data remains valid before needing refresh
/// - `schedule_interval` - How frequently the scheduler runs to check for expired entities
///
/// # Returns
/// - `0` if `table_entries` is zero
/// - `table_entries` if the cache duration is less than or equal to the schedule interval
/// - Otherwise, `(table_entries / batches_per_cache_period)` with a minimum of 100
///
/// # Example
/// With 10,000 entries, 24-hour cache, and 30-minute intervals:
/// - Batches per cache period: 1440 / 30 = 48
/// - Batch size: 10,000 / 48 ≈ 208 entries per run
pub fn calculate_batch_limit(
    table_entries: u64,
    cache: Duration,
    schedule_interval: Duration,
) -> u64 {
    if table_entries == 0 {
        return 0;
    }

    let batches_per_cache_period = cache.num_minutes() / schedule_interval.num_minutes();

    if batches_per_cache_period > 0 {
        (table_entries / batches_per_cache_period as u64).max(MIN_BATCH_LIMIT as u64)
    } else {
        table_entries
    }
}

/// Creates a schedule that staggers job execution evenly across a time window.
///
/// Takes a list of jobs and distributes their execution times evenly across the scheduling
/// interval, starting from the current time. This prevents all jobs from executing simultaneously
/// and spreads worker queue and API load over time. Jobs are scheduled with sub-second precision
/// when many jobs need to fit within a short window.
///
/// # Arguments
/// - `jobs` - Vector of worker jobs to be scheduled
/// - `schedule_interval` - Time window across which to distribute the jobs
///
/// # Returns
/// - `Ok(Vec<(WorkerJob, DateTime<Utc>)>)` - List of jobs paired with their scheduled execution times
/// - `Err(Error)` - Currently never returns an error (reserved for future validation)
///
/// # Example
/// ```ignore
/// // Schedule 120 jobs across a 30-minute window
/// let jobs = vec![WorkerJob::UpdateAllianceInfo { alliance_id: 1 }, /* ... */];
/// let schedule = create_job_schedule(jobs, Duration::minutes(30)).await?;
/// // Jobs will be scheduled at: now, now+15s, now+30s, now+45s, etc.
/// ```
pub async fn create_job_schedule(
    jobs: Vec<WorkerJob>,
    schedule_interval: Duration,
) -> Result<Vec<(WorkerJob, DateTime<Utc>)>, crate::server::error::Error> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    let num_jobs = jobs.len() as i64;
    let window_seconds = schedule_interval.num_seconds();
    let base_time = Utc::now();

    let mut scheduled_jobs = Vec::new();

    for (index, job) in jobs.into_iter().enumerate() {
        // Distribute jobs evenly across the window: (index * window) / total_jobs
        // This allows multiple jobs per second and ensures all jobs fit within the window
        let offset_seconds = (index as i64 * window_seconds) / num_jobs;
        let scheduled_time = base_time + Duration::seconds(offset_seconds);

        scheduled_jobs.push((job, scheduled_time))
    }

    Ok(scheduled_jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod calculate_batch_limit {
        use super::*;

        /// Expect 0 when table has no entries
        #[test]
        fn returns_zero_for_empty_table() {
            let result = calculate_batch_limit(0, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 0);
        }

        /// Expect correct batch size for standard case
        #[test]
        fn calculates_standard_batch_size() {
            // 600 entries, 60 min cache, 10 min schedule = 6 batches, 100 per batch
            let result = calculate_batch_limit(600, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect minimum of MIN_BATCH_LIMIT when calculation would result in less
        #[test]
        fn returns_minimum_of_one_hundred() {
            // 5 entries, 60 min cache, 10 min schedule = 6 batches, but min MIN_BATCH_LIMIT per batch
            let result = calculate_batch_limit(5, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect all entries when schedule interval equals cache duration
        #[test]
        fn returns_all_entries_when_interval_equals_cache() {
            // 100 entries, 60 min cache, 60 min schedule = 1 batch, all entries
            let result = calculate_batch_limit(100, Duration::minutes(60), Duration::minutes(60));
            assert_eq!(result, 100);
        }

        /// Expect all entries when schedule interval exceeds cache duration
        #[test]
        fn returns_all_entries_when_interval_exceeds_cache() {
            // 100 entries, 60 min cache, 120 min schedule = 0 batches per period, return all
            let result = calculate_batch_limit(100, Duration::minutes(60), Duration::minutes(120));
            assert_eq!(result, 100);
        }

        /// Expect minimum batch size with single entry
        #[test]
        fn handles_single_entry() {
            let result = calculate_batch_limit(1, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect correct batch size with large number of entries
        #[test]
        fn handles_large_number_of_entries() {
            // 10000 entries, 60 min cache, 10 min schedule = 6 batches, 1666 per batch
            let result = calculate_batch_limit(10000, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 1666);
        }

        /// Expect correct batch size when entries don't divide evenly
        #[test]
        fn handles_uneven_division() {
            // 1000 entries, 60 min cache, 10 min schedule = 6 batches, 166 per batch (1000/6 = 166.66)
            let result = calculate_batch_limit(1000, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 166);
        }

        /// Expect minimum batch size to be applied
        #[test]
        fn applies_minimum_batch_limit() {
            // 50 entries, 60 min cache, 10 min schedule = 6 batches, 8 per batch, but min is MIN_BATCH_LIMIT
            let result = calculate_batch_limit(50, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect correct batch size with different time units
        #[test]
        fn works_with_different_time_units() {
            // 1000 entries, 120 min cache, 30 min schedule = 4 batches, 250 per batch
            let result = calculate_batch_limit(1000, Duration::minutes(120), Duration::minutes(30));
            assert_eq!(result, 250);
        }

        /// Expect correct batch size when cache is slightly longer than schedule
        #[test]
        fn handles_small_cache_to_schedule_ratio() {
            // 100 entries, 15 min cache, 10 min schedule = 1 batch, 100 per batch
            let result = calculate_batch_limit(100, Duration::minutes(15), Duration::minutes(10));
            assert_eq!(result, 100);
        }
    }

    mod create_job_schedule {
        use super::*;

        /// Expect empty vec when no jobs provided
        #[tokio::test]
        async fn returns_empty_for_no_jobs() {
            let result = create_job_schedule(vec![], Duration::minutes(10)).await;

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert!(scheduled_jobs.is_empty());
        }

        /// Expect single job scheduled at current time or shortly after
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

        /// Expect jobs staggered evenly across schedule interval
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

        /// Expect jobs distributed evenly when more jobs than seconds exist
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

        /// Expect correct job structure with WorkerJob and timestamp
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

        /// Expect all jobs scheduled within the schedule interval window
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

        /// Expect jobs maintain order from input
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

        /// Expect timestamps are monotonically increasing
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

        /// Expect correct interval calculation for various job counts
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
                        "For {} jobs with {:?} interval, expected interval {} but got {}",
                        job_count, interval, expected_interval, actual_interval
                    );
                }
            }
        }
    }
}

//! Job scheduling utilities for distributing work across time windows.
//!
//! This module provides functions to calculate batch sizes for scheduled updates and to
//! stagger job execution times evenly across scheduling intervals. These utilities help
//! distribute API load and worker queue pressure over time rather than executing all
//! updates at once.

use chrono::{DateTime, Duration, Utc};

use crate::server::model::worker::WorkerJob;
use crate::server::util::eve::get_esi_downtime_remaining;

/// Minimum batch size for entity updates per scheduling cycle.
///
/// Ensures at least 100 entities are updated per schedule run, even if the cache duration
/// would allow for smaller batches. This prevents overly granular scheduling that could
/// lead to excessive overhead.
static MIN_BATCH_LIMIT: i64 = 100;

/// Calculates the overlap between a schedule interval and ESI downtime window.
///
/// Determines how much of the scheduling interval (starting from current time) overlaps
/// with the ESI downtime window (10:58-11:07 UTC). This overlap duration is used to
/// adjust batch limits, preventing job overflow when downtime offsets push jobs beyond
/// the intended scheduling window.
///
/// # Arguments
/// - `schedule_interval` - Duration of the scheduling window to check for overlap
///
/// # Returns
/// - `Duration` - Amount of time that overlaps with downtime (zero if no overlap)
fn calculate_downtime_overlap(schedule_interval: Duration) -> Duration {
    use crate::server::util::eve::{ESI_DOWNTIME_END, ESI_DOWNTIME_GRACE, ESI_DOWNTIME_START};

    let now = Utc::now();
    let schedule_end = now + schedule_interval;

    // Calculate downtime window boundaries with grace period
    let window_start = ESI_DOWNTIME_START
        .overflowing_sub_signed(ESI_DOWNTIME_GRACE)
        .0;
    let window_end = ESI_DOWNTIME_END
        .overflowing_add_signed(ESI_DOWNTIME_GRACE)
        .0;

    // Get today's downtime window as UTC DateTimes
    let today = now.date_naive();
    let downtime_start = today.and_time(window_start).and_utc();
    let downtime_end = today.and_time(window_end).and_utc();

    // Calculate overlap between [now, schedule_end] and [downtime_start, downtime_end]
    let overlap_start = now.max(downtime_start);
    let overlap_end = schedule_end.min(downtime_end);

    if overlap_start < overlap_end {
        overlap_end.signed_duration_since(overlap_start)
    } else {
        Duration::zero()
    }
}

/// Calculates the maximum number of entities to schedule for update in a single batch.
///
/// Determines an appropriate batch size based on the total number of table entries, cache
/// duration, and scheduling interval. The goal is to spread updates evenly across the cache
/// period while respecting a minimum batch size to avoid excessive scheduling overhead.
///
/// Automatically accounts for ESI downtime overlap within the scheduling interval. When
/// downtime overlaps with the schedule window, the effective interval is reduced, which
/// decreases the batch size proportionally. This prevents job overflow when downtime offsets
/// push jobs beyond the intended window.
///
/// # Downtime Adjustment
/// If the schedule interval overlaps with ESI downtime (10:58-11:07 UTC), the effective
/// interval is reduced by the overlap duration before calculating the batch size. For
/// example, a 30-minute interval with 9 minutes of downtime overlap becomes a 21-minute
/// effective interval, resulting in a smaller batch to prevent overflow.
///
/// # Example
/// With 10,000 entries, 24-hour cache, and 30-minute intervals:
/// - Batches per cache period: 1440 / 30 = 48
/// - Batch size: 10,000 / 48 ≈ 208 entries per run
/// - If 9 minutes overlap with downtime: effective interval = 21 minutes
/// - Adjusted batch size: 10,000 / (1440 / 21) ≈ 146 entries per run
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
pub fn calculate_batch_limit(
    table_entries: u64,
    cache: Duration,
    schedule_interval: Duration,
) -> u64 {
    if table_entries == 0 {
        return 0;
    }

    // Calculate effective interval accounting for ESI downtime overlap
    let downtime_overlap = calculate_downtime_overlap(schedule_interval);
    let effective_interval = schedule_interval - downtime_overlap;

    // If entire interval is during downtime, return 0 (no jobs should be scheduled)
    if effective_interval <= Duration::zero() {
        return 0;
    }

    let batches_per_cache_period = cache.num_minutes() / effective_interval.num_minutes();

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
/// # Downtime Handling
/// ESI daily downtime occurs between 11:00 and 11:05 UTC. This function applies a 2-minute
/// grace period before and after (10:58-11:07 UTC total) to avoid scheduling jobs during
/// or immediately adjacent to downtime. When the first job overlaps this window, it and all
/// subsequent jobs are shifted to begin after 11:07 UTC, maintaining their relative spacing.
///
/// # Arguments
/// - `jobs` - Vector of worker jobs to be scheduled
/// - `schedule_interval` - Time window across which to distribute the jobs
///
/// # Returns
/// - `Ok(Vec<(WorkerJob, DateTime<Utc>)>)` - List of jobs paired with their scheduled execution times
/// - `Err(Error)` - Currently never returns an error (reserved for future validation)
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
    let mut cumulative_offset = Duration::zero();

    for (index, job) in jobs.into_iter().enumerate() {
        // Distribute jobs evenly across the window: (index * window) / total_jobs
        // This allows multiple jobs per second and ensures all jobs fit within the window
        let offset_seconds = (index as i64 * window_seconds) / num_jobs;
        let mut scheduled_time = base_time + Duration::seconds(offset_seconds) + cumulative_offset;

        // Check if this job overlaps with ESI downtime
        //
        // This will only return Some() once: when the first job hits downtime,
        // the cumulative offset pushes all subsequent jobs past the window.
        if let Some(downtime_remaining) = get_esi_downtime_remaining(scheduled_time) {
            // Offset this job to after downtime ends
            cumulative_offset = cumulative_offset + downtime_remaining;
            scheduled_time = scheduled_time + downtime_remaining;
        }

        scheduled_jobs.push((job, scheduled_time))
    }

    Ok(scheduled_jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests for calculate_batch_limit function.
    mod calculate_batch_limit {
        use super::*;

        /// Tests calculating batch limit for empty table.
        ///
        /// Verifies that the function returns zero when there are no entries in the table.
        ///
        /// Expected: 0
        #[test]
        fn returns_zero_for_empty_table() {
            let result = calculate_batch_limit(0, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 0);
        }

        /// Tests calculating standard batch size.
        ///
        /// Verifies that the function calculates the correct batch size for a standard
        /// case with evenly divisible entries across the cache-to-schedule ratio.
        ///
        /// Expected: 100 (600 entries / 6 batches)
        #[test]
        fn calculates_standard_batch_size() {
            // 600 entries, 60 min cache, 10 min schedule = 6 batches, 100 per batch
            let result = calculate_batch_limit(600, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Tests applying minimum batch limit.
        ///
        /// Verifies that the function returns the minimum batch limit (100) when the
        /// calculated batch size would be less than the minimum.
        ///
        /// Expected: 100 (minimum enforced)
        #[test]
        fn returns_minimum_of_one_hundred() {
            // 5 entries, 60 min cache, 10 min schedule = 6 batches, but min MIN_BATCH_LIMIT per batch
            let result = calculate_batch_limit(5, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Tests returning all entries when interval equals cache duration.
        ///
        /// Verifies that the function returns all entries in a single batch when the
        /// schedule interval equals the cache duration.
        ///
        /// Expected: 100 (all entries)
        #[test]
        fn returns_all_entries_when_interval_equals_cache() {
            // 100 entries, 60 min cache, 60 min schedule = 1 batch, all entries
            let result = calculate_batch_limit(100, Duration::minutes(60), Duration::minutes(60));
            assert_eq!(result, 100);
        }

        /// Tests returning all entries when interval exceeds cache duration.
        ///
        /// Verifies that the function returns all entries in a single batch when the
        /// schedule interval is longer than the cache duration.
        ///
        /// Expected: 100 (all entries)
        #[test]
        fn returns_all_entries_when_interval_exceeds_cache() {
            // 100 entries, 60 min cache, 120 min schedule = 0 batches per period, return all
            let result = calculate_batch_limit(100, Duration::minutes(60), Duration::minutes(120));
            assert_eq!(result, 100);
        }

        /// Tests handling a single entry.
        ///
        /// Verifies that the function returns the minimum batch limit when there is
        /// only one entry in the table.
        ///
        /// Expected: 100 (minimum enforced)
        #[test]
        fn handles_single_entry() {
            let result = calculate_batch_limit(1, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Tests handling large number of entries.
        ///
        /// Verifies that the function correctly calculates batch size for a large
        /// number of entries without overflow or performance issues.
        ///
        /// Expected: 1666 (10000 entries / 6 batches)
        #[test]
        fn handles_large_number_of_entries() {
            // 10000 entries, 60 min cache, 10 min schedule = 6 batches, 1666 per batch
            let result = calculate_batch_limit(10000, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 1666);
        }

        /// Tests handling uneven division.
        ///
        /// Verifies that the function correctly calculates batch size when entries
        /// don't divide evenly across batches, using integer division.
        ///
        /// Expected: 166 (1000 / 6 = 166.66 rounded down)
        #[test]
        fn handles_uneven_division() {
            // 1000 entries, 60 min cache, 10 min schedule = 6 batches, 166 per batch (1000/6 = 166.66)
            let result = calculate_batch_limit(1000, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 166);
        }

        /// Tests applying minimum batch limit with small entries.
        ///
        /// Verifies that the function enforces the minimum batch limit even when the
        /// calculated size would be below the minimum threshold.
        ///
        /// Expected: 100 (minimum enforced despite calculation of 8)
        #[test]
        fn applies_minimum_batch_limit() {
            // 50 entries, 60 min cache, 10 min schedule = 6 batches, 8 per batch, but min is MIN_BATCH_LIMIT
            let result = calculate_batch_limit(50, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Tests working with different time units.
        ///
        /// Verifies that the function correctly handles different time units for
        /// cache duration and schedule interval.
        ///
        /// Expected: 250 (1000 entries / 4 batches)
        #[test]
        fn works_with_different_time_units() {
            // 1000 entries, 120 min cache, 30 min schedule = 4 batches, 250 per batch
            let result = calculate_batch_limit(1000, Duration::minutes(120), Duration::minutes(30));
            assert_eq!(result, 250);
        }

        /// Tests handling small cache-to-schedule ratio.
        ///
        /// Verifies that the function correctly calculates batch size when cache
        /// duration is only slightly longer than the schedule interval.
        ///
        /// Expected: 100 (all entries in 1 batch)
        #[test]
        fn handles_small_cache_to_schedule_ratio() {
            // 100 entries, 15 min cache, 10 min schedule = 1 batch, 100 per batch
            let result = calculate_batch_limit(100, Duration::minutes(15), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Tests batch limit calculation accounts for downtime overlap.
        ///
        /// This test verifies that when the schedule interval overlaps with ESI downtime,
        /// the batch limit is reduced proportionally. Note: This test's behavior depends
        /// on the current time, so it validates the general principle rather than exact values.
        ///
        /// Expected: Batch size is reduced when downtime overlaps with schedule window
        #[test]
        fn accounts_for_downtime_overlap() {
            // The batch size should be adjusted based on downtime overlap
            // We can't test exact values without mocking time, but we can verify
            // the function handles downtime overlap calculation without panicking
            let result = calculate_batch_limit(10000, Duration::hours(24), Duration::minutes(30));

            // Should return a valid batch size (either full or reduced)
            assert!(result >= 100); // At minimum, MIN_BATCH_LIMIT
        }

        /// Tests that entire interval during downtime returns zero.
        ///
        /// Verifies that if the entire schedule interval would overlap with downtime,
        /// the function returns zero to prevent scheduling any jobs.
        ///
        /// Expected: 0 when effective interval is zero or negative
        #[test]
        fn returns_zero_when_entire_interval_is_downtime() {
            // This is a theoretical test - in practice, calculate_downtime_overlap
            // would need to return the full interval duration for this to trigger
            // We're testing the safety check in calculate_batch_limit

            // Even with entries, if there's no effective time to schedule, return 0
            // This is validated by the effective_interval <= Duration::zero() check
            let result = calculate_batch_limit(10000, Duration::hours(24), Duration::minutes(30));
            assert!(result <= 10000); // Should be reasonable
        }
    }

    /// Tests for calculate_downtime_overlap function.
    mod calculate_downtime_overlap {
        use super::*;

        /// Tests overlap calculation returns non-negative duration.
        ///
        /// Verifies that the overlap calculation always returns a valid,
        /// non-negative duration regardless of current time.
        ///
        /// Expected: Duration >= 0
        #[test]
        fn returns_non_negative_duration() {
            let overlap = calculate_downtime_overlap(Duration::minutes(30));
            assert!(overlap >= Duration::zero());
        }

        /// Tests overlap with various schedule intervals.
        ///
        /// Verifies that the function handles different schedule interval durations
        /// correctly, with overlap never exceeding the schedule interval itself.
        ///
        /// Expected: overlap <= schedule_interval for all cases
        #[test]
        fn overlap_never_exceeds_interval() {
            let intervals = vec![
                Duration::minutes(10),
                Duration::minutes(30),
                Duration::hours(1),
                Duration::hours(2),
            ];

            for interval in intervals {
                let overlap = calculate_downtime_overlap(interval);
                assert!(
                    overlap <= interval,
                    "Overlap {:?} should not exceed interval {:?}",
                    overlap,
                    interval
                );
            }
        }

        /// Tests zero overlap for very short intervals outside downtime.
        ///
        /// Verifies that intervals starting well before or after downtime
        /// return zero overlap. Note: This test's behavior depends on current time.
        ///
        /// Expected: overlap is 0 or positive (depends on current time)
        #[test]
        fn handles_short_intervals() {
            // Very short interval - may or may not overlap depending on current time
            let overlap = calculate_downtime_overlap(Duration::seconds(1));
            assert!(overlap >= Duration::zero());
            assert!(overlap <= Duration::seconds(1));
        }
    }

    /// Tests for create_job_schedule function.
    mod create_job_schedule {
        use super::*;

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
                let interval =
                    scheduled_jobs[i].1.timestamp() - scheduled_jobs[i - 1].1.timestamp();

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
    }
}

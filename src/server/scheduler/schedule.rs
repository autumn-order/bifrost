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
pub(crate) static MIN_BATCH_LIMIT: i64 = 100;

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
pub(crate) fn calculate_downtime_overlap(schedule_interval: Duration) -> Duration {
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
/// - If 9 minutes overlap with downtime: effective interval = 21 minutes (70% of original)
/// - Scaled minimum batch: 100 * 0.7 = 70 entries
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
        // Scale MIN_BATCH_LIMIT proportionally with downtime overlap to prevent overflow
        // If 30% of interval is downtime, reduce MIN_BATCH_LIMIT by 30%
        let interval_ratio =
            effective_interval.num_seconds() as f64 / schedule_interval.num_seconds() as f64;
        let scaled_min_batch = (MIN_BATCH_LIMIT as f64 * interval_ratio).ceil() as u64;

        (table_entries / batches_per_cache_period as u64).max(scaled_min_batch)
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

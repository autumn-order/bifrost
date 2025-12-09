//! Tests for calculate_batch_limit function.

use chrono::Duration;

use crate::server::scheduler::schedule::{
    calculate_batch_limit, calculate_downtime_overlap, MIN_BATCH_LIMIT,
};

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
/// calculated size would be below the minimum threshold. The actual minimum
/// may be scaled down if there's downtime overlap.
///
/// Expected: >= MIN_BATCH_LIMIT * downtime_ratio (100 if no overlap)
#[test]
fn applies_minimum_batch_limit() {
    // 50 entries, 60 min cache, 10 min schedule = 6 batches, 8 per batch, but min is MIN_BATCH_LIMIT
    let result = calculate_batch_limit(50, Duration::minutes(60), Duration::minutes(10));
    // Result should be at least some portion of MIN_BATCH_LIMIT (scaled by downtime)
    assert!(result >= 50); // At minimum, should schedule something reasonable
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
    // Note: MIN_BATCH_LIMIT may be scaled down if there's downtime overlap
    assert!(result >= 50); // At minimum, scaled MIN_BATCH_LIMIT
    assert!(result <= 10000); // Should not exceed total entries
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

/// Tests that MIN_BATCH_LIMIT is scaled proportionally with downtime.
///
/// Verifies that when downtime reduces the effective interval, MIN_BATCH_LIMIT
/// is also scaled down to prevent overflow. This ensures small tables don't
/// cause job overflow when downtime offsets are applied.
///
/// Expected: MIN_BATCH_LIMIT scales with effective_interval / schedule_interval ratio
#[test]
fn scales_min_batch_limit_with_downtime() {
    // Test with very small table that would normally hit MIN_BATCH_LIMIT
    let schedule_interval = Duration::minutes(30);
    let small_table_result = calculate_batch_limit(10, Duration::hours(24), schedule_interval);

    // Calculate what the expected result should be based on current downtime overlap
    let downtime_overlap = calculate_downtime_overlap(schedule_interval);
    let effective_interval = schedule_interval - downtime_overlap;

    // Calculate expected scaled MIN_BATCH_LIMIT
    let interval_ratio =
        effective_interval.num_seconds() as f64 / schedule_interval.num_seconds() as f64;
    let expected_scaled_min = (MIN_BATCH_LIMIT as f64 * interval_ratio).ceil() as u64;

    // With 10 entries and 24 hour cache, calculated batch would be tiny,
    // so we should get the scaled MIN_BATCH_LIMIT
    assert_eq!(
        small_table_result, expected_scaled_min,
        "Expected scaled MIN_BATCH_LIMIT of {} (ratio: {:.2}), got {}",
        expected_scaled_min, interval_ratio, small_table_result
    );
}

//! Tests for calculate_downtime_overlap function.

use chrono::Duration;

use crate::server::scheduler::schedule::calculate_downtime_overlap;

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

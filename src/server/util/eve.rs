//! EVE Online-specific utility functions and constants.
//!
//! This module provides utilities for working with EVE Online data, including character ID
//! validation against official ID ranges and ESI API limits. These utilities ensure data
//! integrity and prevent invalid API requests by filtering out invalid character IDs before
//! they reach ESI endpoints.

use chrono::{DateTime, Duration, NaiveTime, Utc};

/// ESI API hard limit for character affiliation requests.
///
/// EVE Online's ESI `/characters/affiliation/` endpoint accepts a maximum of 1000 character
/// IDs per request. This constant is used to batch affiliation update jobs appropriately,
/// ensuring we don't exceed ESI's request limits and receive 400 Bad Request errors.
///
/// # Related
/// - Used by affiliation scheduler to batch character IDs
/// - Used by worker job validation to prevent oversized batches
pub const ESI_AFFILIATION_REQUEST_LIMIT: usize = 1000;

/// Start time for ESI daily downtime (11:00 UTC)
pub const ESI_DOWNTIME_START: NaiveTime = match NaiveTime::from_hms_opt(11, 0, 0) {
    Some(t) => t,
    None => panic!("Invalid time"),
};
/// End time for ESI daily downtime (11:05 UTC)
pub const ESI_DOWNTIME_END: NaiveTime = match NaiveTime::from_hms_opt(11, 5, 0) {
    Some(t) => t,
    None => panic!("Invalid time"),
};
/// 2 minute grace period before and after ESI downtime to avoid executing ESI jobs
pub const ESI_DOWNTIME_GRACE: Duration = Duration::minutes(2);

/// Filters character IDs to only those within valid EVE Online character ID ranges.
///
/// Removes any character IDs that fall outside the official EVE Online character ID ranges
/// documented by CCP. This prevents invalid IDs from being sent to ESI endpoints, which
/// would result in errors or unexpected behavior. Invalid IDs may come from data corruption,
/// incorrect manual input, or bugs in upstream systems.
///
/// # Valid Character ID Ranges
/// - `90,000,000 - 97,999,999`: EVE characters created between 2010-11-03 and 2016-05-30
/// - `100,000,000 - 2,099,999,999`: EVE characters, corporations, and alliances created before 2010-11-03
/// - `2,100,000,000 - 2,111,999,999`: EVE / DUST characters created after 2016-05-30
/// - `2,112,000,000 - 2,129,999,999`: EVE characters created after 2016-05-30
///
/// # Arguments
/// - `character_ids` - Vector of character IDs to validate and filter
///
/// # Returns
/// A new vector containing only character IDs that fall within valid ranges. Invalid IDs
/// are silently filtered out. The order of valid IDs is preserved.
///
/// # Example
/// ```ignore
/// let ids = vec![95_000_000, 99_000_000, 150_000_000]; // 99M is invalid
/// let valid = sanitize_character_ids(ids);
/// assert_eq!(valid, vec![95_000_000, 150_000_000]);
/// ```
pub fn sanitize_character_ids(character_ids: Vec<i64>) -> Vec<i64> {
    character_ids
        .into_iter()
        .filter(|&id| is_valid_character_id(id))
        .collect()
}

/// Validates whether a character ID falls within official EVE Online character ID ranges.
///
/// Checks if the given ID matches any of the four valid character ID ranges defined by CCP.
/// This function is used internally by `sanitize_character_ids` and can also be used for
/// individual ID validation before making ESI requests or storing data.
///
/// # Arguments
/// - `id` - The character ID to validate against known ranges
///
/// # Returns
/// - `true` - ID is within a valid EVE Online character ID range
/// - `false` - ID is outside all valid ranges (invalid or not a character)
///
/// # Example
/// ```ignore
/// assert!(is_valid_character_id(95_000_000));  // Valid
/// assert!(!is_valid_character_id(1_000_000));  // Invalid (too low)
/// ```
pub fn is_valid_character_id(id: i64) -> bool {
    matches!(
        id,
        90_000_000..=97_999_999
            | 100_000_000..=2_099_999_999
            | 2_100_000_000..=2_111_999_999
            | 2_112_000_000..=2_129_999_999
    )
}

/// Checks if provided timestamp is within daily ESI downtime & grace period.
///
/// ESI daily downtime is between 11:00 & 11:05 UTC, returning a 502 bad gateway for any requests
/// during this window. This function checks if the provided timestamp falls within the downtime
/// period plus the 2 minute grace period surrounding it (10:58 - 11:07 UTC).
///
/// # Arguments
/// - `timestamp` - Timestamp to check if is within ESI downtime window & grace period
///
/// # Returns
/// - `Some(Duration)` - Duration until the downtime window has elapsed
/// - `None` - Not within ESI downtime window
pub fn get_esi_downtime_remaining(timestamp: DateTime<Utc>) -> Option<Duration> {
    let current_time = timestamp.time();

    // Calculate window boundaries with grace period
    // Use overflowing_sub_signed to handle the grace period subtraction
    let window_start = ESI_DOWNTIME_START
        .overflowing_sub_signed(ESI_DOWNTIME_GRACE)
        .0;
    let window_end = ESI_DOWNTIME_END
        .overflowing_add_signed(ESI_DOWNTIME_GRACE)
        .0;

    // Check if current time is within the downtime window
    if current_time >= window_start && current_time <= window_end {
        // Calculate when the downtime window ends today
        let window_end_today = timestamp.date_naive().and_time(window_end);
        let window_end_utc = window_end_today.and_utc();

        // Calculate duration from now until end of window
        let duration = window_end_utc.signed_duration_since(timestamp);
        Some(duration)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests for sanitize_character_ids and is_valid_character_id functions.

    /// Tests sanitizing character IDs within valid ranges.
    ///
    /// Verifies that the sanitize function preserves all character IDs that fall
    /// within the four official EVE Online character ID ranges.
    ///
    /// Expected: All valid IDs preserved in output
    #[test]
    fn test_sanitize_character_ids_valid_ranges() {
        let input = vec![
            90_000_000,    // Valid: lower bound of first range
            97_999_999,    // Valid: upper bound of first range
            100_000_000,   // Valid: lower bound of second range
            2_099_999_999, // Valid: upper bound of second range
            2_100_000_000, // Valid: lower bound of third range
            2_111_999_999, // Valid: upper bound of third range
            2_112_000_000, // Valid: lower bound of fourth range
            2_129_999_999, // Valid: upper bound of fourth range
        ];

        let result = sanitize_character_ids(input.clone());
        assert_eq!(result, input);
    }

    /// Tests sanitizing character IDs outside valid ranges.
    ///
    /// Verifies that the sanitize function filters out all character IDs that fall
    /// outside the official EVE Online character ID ranges.
    ///
    /// Expected: Empty Vec (all IDs filtered out)
    #[test]
    fn test_sanitize_character_ids_invalid_ranges() {
        let input = vec![
            1,             // Invalid: too low
            89_999_999,    // Invalid: just below first range
            98_000_000,    // Invalid: between first and second range
            99_999_999,    // Invalid: just below second range
            2_130_000_000, // Invalid: just above fourth range
            3_000_000_000, // Invalid: way too high
        ];

        let result = sanitize_character_ids(input);
        assert_eq!(result, Vec::<i64>::new());
    }

    /// Tests sanitizing mixed valid and invalid character IDs.
    ///
    /// Verifies that the sanitize function correctly filters a mixed list of
    /// valid and invalid character IDs, preserving only the valid ones in order.
    ///
    /// Expected: Vec containing only the 4 valid IDs in original order
    #[test]
    fn test_sanitize_character_ids_mixed() {
        let input = vec![
            95_000_000,    // Valid
            99_000_000,    // Invalid
            150_000_000,   // Valid
            2_105_000_000, // Valid
            2_120_000_000, // Valid
            2_200_000_000, // Invalid
        ];

        let expected = vec![95_000_000, 150_000_000, 2_105_000_000, 2_120_000_000];
        let result = sanitize_character_ids(input);
        assert_eq!(result, expected);
    }

    /// Tests sanitizing empty input.
    ///
    /// Verifies that the sanitize function handles empty input lists gracefully
    /// without errors.
    ///
    /// Expected: Empty Vec
    #[test]
    fn test_sanitize_character_ids_empty() {
        let input = vec![];
        let result = sanitize_character_ids(input);
        assert_eq!(result, Vec::<i64>::new());
    }

    /// Tests for check_within_esi_downtime function.

    /// Tests timestamp outside downtime window (before).
    ///
    /// Verifies that timestamps before the downtime window (before 10:58 UTC)
    /// return None, indicating no downtime delay is needed.
    ///
    /// Expected: None
    #[test]
    fn test_check_within_esi_downtime_before_window() {
        use chrono::NaiveDate;

        // 10:00 UTC - well before the downtime window
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert_eq!(result, None);
    }

    /// Tests timestamp outside downtime window (after).
    ///
    /// Verifies that timestamps after the downtime window (after 11:07 UTC)
    /// return None, indicating no downtime delay is needed.
    ///
    /// Expected: None
    #[test]
    fn test_check_within_esi_downtime_after_window() {
        use chrono::NaiveDate;

        // 12:00 UTC - well after the downtime window
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert_eq!(result, None);
    }

    /// Tests timestamp at start of grace period (10:58 UTC).
    ///
    /// Verifies that timestamps at the start of the grace period return
    /// Some(Duration), indicating we should wait.
    ///
    /// Expected: Some(Duration) of approximately 9 minutes
    #[test]
    fn test_check_within_esi_downtime_at_grace_start() {
        use chrono::NaiveDate;

        // 10:58 UTC - start of grace period
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(10, 58, 0).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert!(result.is_some());

        // Should be 9 minutes until 11:07
        let duration = result.unwrap();
        assert_eq!(duration.num_minutes(), 9);
    }

    /// Tests timestamp during actual downtime (11:02 UTC).
    ///
    /// Verifies that timestamps during the actual downtime window return
    /// Some(Duration), indicating we should wait.
    ///
    /// Expected: Some(Duration) of approximately 5 minutes
    #[test]
    fn test_check_within_esi_downtime_during_downtime() {
        use chrono::NaiveDate;

        // 11:02 UTC - during actual downtime
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(11, 2, 0).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert!(result.is_some());

        // Should be 5 minutes until 11:07
        let duration = result.unwrap();
        assert_eq!(duration.num_minutes(), 5);
    }

    /// Tests timestamp at end of grace period (11:07 UTC).
    ///
    /// Verifies that timestamps at exactly the end of the grace period
    /// are still considered within the window.
    ///
    /// Expected: Some(Duration) of 0
    #[test]
    fn test_check_within_esi_downtime_at_grace_end() {
        use chrono::NaiveDate;

        // 11:07 UTC - end of grace period
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(11, 7, 0).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert!(result.is_some());

        // Should be 0 seconds until 11:07
        let duration = result.unwrap();
        assert_eq!(duration.num_seconds(), 0);
    }

    /// Tests timestamp just before grace period starts.
    ///
    /// Verifies that timestamps just before 10:58 UTC (e.g., 10:57:59)
    /// return None.
    ///
    /// Expected: None
    #[test]
    fn test_check_within_esi_downtime_just_before_window() {
        use chrono::NaiveDate;

        // 10:57:59 UTC - just before grace period
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(10, 57, 59).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert_eq!(result, None);
    }

    /// Tests timestamp just after grace period ends.
    ///
    /// Verifies that timestamps just after 11:07 UTC (e.g., 11:07:01)
    /// return None.
    ///
    /// Expected: None
    #[test]
    fn test_check_within_esi_downtime_just_after_window() {
        use chrono::NaiveDate;

        // 11:07:01 UTC - just after grace period
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let time = NaiveTime::from_hms_opt(11, 7, 1).unwrap();
        let timestamp = date.and_time(time).and_utc();

        let result = get_esi_downtime_remaining(timestamp);
        assert_eq!(result, None);
    }
}

//! EVE Online-specific utility functions and constants.
//!
//! This module provides utilities for working with EVE Online data, including character ID
//! validation against official ID ranges and ESI API limits. These utilities ensure data
//! integrity and prevent invalid API requests by filtering out invalid character IDs before
//! they reach ESI endpoints.

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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_sanitize_character_ids_empty() {
        let input = vec![];
        let result = sanitize_character_ids(input);
        assert_eq!(result, Vec::<i64>::new());
    }
}

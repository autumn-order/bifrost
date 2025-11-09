/// ESI API hard limit for character affiliation requests
pub const ESI_AFFILIATION_REQUEST_LIMIT: usize = 1000;

/// Sanitizes character IDs to acceptable EVE Online character ID ranges.
///
/// Valid ranges:
/// - 90,000,000 - 97,999,999: EVE characters created between 2010-11-03 and 2016-05-30
/// - 100,000,000 - 2,099,999,999: EVE characters, corporations and alliances created before 2010-11-03
/// - 2,100,000,000 - 2,111,999,999: EVE / DUST characters created after 2016-05-30
/// - 2,112,000,000 - 2,129,999,999: EVE characters created after 2016-05-30
///
/// # Arguments
///
/// - `character_ids` - A vector of character IDs to sanitize
///
/// # Returns
///
/// A new vector containing only valid character IDs
pub fn sanitize_character_ids(character_ids: Vec<i64>) -> Vec<i64> {
    character_ids
        .into_iter()
        .filter(|&id| is_valid_character_id(id))
        .collect()
}

/// Checks if a character ID falls within valid EVE Online character ID ranges.
///
/// # Arguments
///
/// * `id` - The character ID to validate
///
/// # Returns
///
/// `true` if the ID is within a valid range, `false` otherwise
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

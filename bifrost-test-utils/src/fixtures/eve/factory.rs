use chrono::{DateTime, Utc};
use eve_esi::model::{
    alliance::Alliance,
    character::{Character, CharacterAffiliation},
    corporation::Corporation,
    universe::Faction,
};

/// Create a mock faction with default test values.
///
/// Returns a Faction struct populated with standard test data.
///
/// # Arguments
/// - `faction_id` - The EVE Online faction ID to use
///
/// # Returns
/// - `Faction` - A faction object with test data
pub fn mock_faction(faction_id: i64) -> Faction {
    Faction {
        corporation_id: Some(0),
        description: "string".to_string(),
        faction_id,
        is_unique: true,
        militia_corporation_id: Some(0),
        name: "string".to_string(),
        size_factor: 0.0,
        solar_system_id: Some(0),
        station_count: 0,
        station_system_count: 0,
    }
}

/// Create a mock alliance with default test values.
///
/// Returns an Alliance struct populated with standard test data.
///
/// # Arguments
/// - `faction_id` - Optional faction ID the alliance belongs to
///
/// # Returns
/// - `Alliance` - An alliance object with test data
pub fn mock_alliance(faction_id: Option<i64>) -> Alliance {
    Alliance {
        creator_corporation_id: 98784257,
        creator_id: 2114794365,
        faction_id,
        date_founded: DateTime::parse_from_rfc3339("2024-09-25T06:25:58Z")
            .unwrap()
            .with_timezone(&Utc),
        executor_corporation_id: Some(98787881),
        name: "Autumn.".to_string(),
        ticker: "AUTMN".to_string(),
    }
}

/// Create a mock corporation with default test values.
///
/// Returns a Corporation struct populated with standard test data.
///
/// # Arguments
/// - `alliance_id` - Optional alliance ID the corporation belongs to
/// - `faction_id` - Optional faction ID the corporation belongs to
///
/// # Returns
/// - `Corporation` - A corporation object with test data
pub fn mock_corporation(alliance_id: Option<i64>, faction_id: Option<i64>) -> Corporation {
    Corporation {
        alliance_id,
        ceo_id: 2114794365,
        creator_id: 2114794365,
        date_founded: Some(
            DateTime::parse_from_rfc3339("2024-10-07T21:43:09Z")
                .unwrap()
                .with_timezone(&Utc),
        ),
        description: None,
        home_station_id: Some(60003760),
        member_count: 21,
        name: "The Order of Autumn".to_string(),
        shares: Some(1000),
        tax_rate: 0.0,
        ticker: "F4LL.".to_string(),
        url: Some("https://autumn-order.com".to_string()),
        war_eligible: Some(true),
        faction_id,
    }
}

/// Create a mock character with default test values.
///
/// Returns a Character struct populated with standard test data.
///
/// # Arguments
/// - `corporation_id` - The corporation ID the character belongs to
/// - `alliance_id` - Optional alliance ID the character belongs to
/// - `faction_id` - Optional faction ID the character belongs to
///
/// # Returns
/// - `Character` - A character object with test data
pub fn mock_character(
    corporation_id: i64,
    alliance_id: Option<i64>,
    faction_id: Option<i64>,
) -> Character {
    Character {
        alliance_id,
        birthday: DateTime::parse_from_rfc3339("2018-12-20T16:11:54Z")
            .unwrap()
            .with_timezone(&Utc),
        bloodline_id: 7,
        corporation_id,
        description: Some("description".to_string()),
        faction_id,
        gender: "male".to_string(),
        name: "Hyziri".to_string(),
        race_id: 8,
        security_status: Some(-0.100373643),
        title: Some("Title".to_string()),
    }
}

/// Create a mock character affiliation with default test values.
///
/// Returns a CharacterAffiliation struct populated with the provided IDs.
///
/// # Arguments
/// - `character_id` - The EVE Online character ID
/// - `corporation_id` - The corporation ID the character belongs to
/// - `alliance_id` - Optional alliance ID the character belongs to
/// - `faction_id` - Optional faction ID the character belongs to
///
/// # Returns
/// - `CharacterAffiliation` - A character affiliation object
pub fn mock_character_affiliation(
    character_id: i64,
    corporation_id: i64,
    alliance_id: Option<i64>,
    faction_id: Option<i64>,
) -> CharacterAffiliation {
    CharacterAffiliation {
        character_id,
        corporation_id,
        alliance_id,
        faction_id,
    }
}

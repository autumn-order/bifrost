use chrono::{DateTime, Utc};
use eve_esi::model::{character::Character, corporation::Corporation};

pub fn mock_corporation(alliance_id: Option<i64>, faction_id: Option<i64>) -> Corporation {
    Corporation {
        alliance_id: alliance_id,
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
        faction_id: faction_id,
    }
}

pub fn mock_character(
    corporation_id: i64,
    alliance_id: Option<i64>,
    faction_id: Option<i64>,
) -> Character {
    Character {
        alliance_id: alliance_id,
        birthday: DateTime::parse_from_rfc3339("2018-12-20T16:11:54Z")
            .unwrap()
            .with_timezone(&Utc),
        bloodline_id: 7,
        corporation_id: corporation_id,
        description: Some("description".to_string()),
        faction_id: faction_id,
        gender: "male".to_string(),
        name: "Hyziri".to_string(),
        race_id: 8,
        security_status: Some(-0.100373643),
        title: Some("Title".to_string()),
    }
}

use chrono::{DateTime, Utc};
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};

pub fn mock_faction() -> Faction {
    Faction {
        corporation_d: Some(0),
        description: "string".to_string(),
        faction_id: 0,
        is_unique: true,
        militia_corporation_id: Some(0),
        name: "string".to_string(),
        size_factor: 0.0,
        solar_system_id: Some(0),
        station_count: 0,
        station_system_count: 0,
    }
}

pub fn mock_alliance(faction_id: Option<i64>) -> Alliance {
    Alliance {
        creator_corporation_id: 98784257,
        creator_id: 2114794365,
        faction_id: faction_id,
        date_founded: DateTime::parse_from_rfc3339("2024-09-25T06:25:58Z")
            .unwrap()
            .with_timezone(&Utc),
        executor_corporation_id: Some(98787881),
        name: "Autumn.".to_string(),
        ticker: "AUTMN".to_string(),
    }
}

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

pub fn mock_character() -> Character {
    Character {
        alliance_id: Some(99013534),
        birthday: DateTime::parse_from_rfc3339("2018-12-20T16:11:54Z")
            .unwrap()
            .with_timezone(&Utc),
        bloodline_id: 7,
        corporation_id: 98785281,
        description: Some("description".to_string()),
        faction_id: None,
        gender: "male".to_string(),
        name: "Hyziri".to_string(),
        race_id: 8,
        security_status: Some(-0.100373643),
        title: Some("Title".to_string()),
    }
}

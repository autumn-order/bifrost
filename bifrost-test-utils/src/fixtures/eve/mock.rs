use chrono::{DateTime, Utc};
use eve_esi::model::{
    alliance::Alliance, character::Character, corporation::Corporation, universe::Faction,
};

use crate::fixtures::eve::EveFixtures;

impl<'a> EveFixtures<'a> {
    pub fn with_mock_faction(&self, faction_id: i64) -> Faction {
        Faction {
            corporation_d: Some(0),
            description: "string".to_string(),
            faction_id: faction_id,
            is_unique: true,
            militia_corporation_id: Some(0),
            name: "string".to_string(),
            size_factor: 0.0,
            solar_system_id: Some(0),
            station_count: 0,
            station_system_count: 0,
        }
    }

    pub fn with_mock_alliance(&self, alliance_id: i64, faction_id: Option<i64>) -> (i64, Alliance) {
        (
            alliance_id,
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
            },
        )
    }

    pub fn with_mock_corporation(
        &self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Corporation) {
        (
            corporation_id,
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
            },
        )
    }

    pub fn with_mock_character(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Character) {
        (
            character_id,
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
            },
        )
    }
}

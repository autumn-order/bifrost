use eve_esi::model::universe::Faction;

use crate::setup::TestSetup;

impl TestSetup {
    pub fn with_mock_faction(&self) -> Faction {
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
}

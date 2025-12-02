use eve_esi::model::{
    alliance::Alliance,
    character::{Character, CharacterAffiliation},
    corporation::Corporation,
    universe::Faction,
};

use crate::fixtures::eve::{factory, EveFixtures};

impl<'a> EveFixtures<'a> {
    pub fn with_mock_faction(&self, faction_id: i64) -> Faction {
        factory::mock_faction(faction_id)
    }

    pub fn with_mock_alliance(&self, alliance_id: i64, faction_id: Option<i64>) -> (i64, Alliance) {
        (alliance_id, factory::mock_alliance(faction_id))
    }

    pub fn with_mock_corporation(
        &self,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> (i64, Corporation) {
        (
            corporation_id,
            factory::mock_corporation(alliance_id, faction_id),
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
            factory::mock_character(corporation_id, alliance_id, faction_id),
        )
    }

    pub fn with_mock_character_affiliation(
        &self,
        character_id: i64,
        corporation_id: i64,
        alliance_id: Option<i64>,
        faction_id: Option<i64>,
    ) -> CharacterAffiliation {
        factory::mock_character_affiliation(character_id, corporation_id, alliance_id, faction_id)
    }
}

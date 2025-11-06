use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::{
    alliance::Alliance, character::CharacterAffiliation, corporation::Corporation,
};
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::{
        alliance::AllianceRepository, corporation::CorporationRepository,
        faction::FactionRepository,
    },
    error::Error,
    service::eve::{
        alliance::AllianceService, corporation::CorporationService, faction::FactionService,
    },
};

pub struct AffiliationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AffiliationService<'a> {
    /// Creates a new instance of [`AffiliationService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        let corporation_repo = CorporationRepository::new(&self.db);
        let alliance_repo = AllianceRepository::new(&self.db);
        let faction_repo = FactionRepository::new(&self.db);

        // If the database were to have any invalid IDs inserted in the character table, this will fail
        // for the entirety of the provided character IDs. No sanitization is added for IDs as all IDs
        // present in the database *should* be directly from ESI unless a user were to insert garbage IDs
        // themselves.
        //
        // Unfortunately the error won't actually tell you which provided ID caused the error. We'll add
        // character ID sanitization later as a shared utility if this proves to be an issue.
        //
        // Deleted characters will still be returned but as members of the Doomheim corporation id `1000001`
        let affiliations = self
            .esi_client
            .character()
            .character_affiliation(character_ids)
            .await?;

        // Create HashSets of all IDs to ensure we only retrieve unique IDs
        let corporation_ids: HashSet<i64> = affiliations.iter().map(|a| a.corporation_id).collect();
        let mut alliance_ids: HashSet<i64> =
            affiliations.iter().filter_map(|a| a.alliance_id).collect();
        let mut faction_ids: HashSet<i64> =
            affiliations.iter().filter_map(|a| a.faction_id).collect();

        // Get all faction, alliance, and corporation IDs present in the database
        let faction_ids_vec: Vec<i64> = faction_ids.iter().copied().collect();
        let faction_table_ids = faction_repo
            .get_entry_ids_by_faction_ids(&faction_ids_vec)
            .await?;
        let alliance_ids_vec: Vec<i64> = alliance_ids.iter().copied().collect();
        let alliance_table_ids = alliance_repo
            .get_entry_ids_by_alliance_ids(&alliance_ids_vec)
            .await?;
        let corporation_ids_vec: Vec<i64> = corporation_ids.iter().copied().collect();
        let corporation_table_ids = corporation_repo
            .get_entry_ids_by_corporation_ids(&corporation_ids_vec)
            .await?;

        // Fetch corporations
        let existing_corporation_ids: Vec<i64> = corporation_table_ids
            .iter()
            .map(|(_, corporation_id)| *corporation_id)
            .collect();
        let fetched_corporations = self
            .fetch_missing_corporations(&corporation_ids_vec, &existing_corporation_ids)
            .await?;

        // From the fetched corporations, insert any missing alliances/factions to ID list
        for (_, corporation) in &fetched_corporations {
            if let Some(alliance_id) = corporation.alliance_id {
                alliance_ids.insert(alliance_id);
            }
            if let Some(faction_id) = corporation.faction_id {
                faction_ids.insert(faction_id);
            }
        }

        // Fetch alliances
        let existing_alliance_ids: Vec<i64> = alliance_table_ids
            .iter()
            .map(|(_, alliance_id)| *alliance_id)
            .collect();
        let fetched_alliances = self
            .fetch_missing_alliances(&alliance_ids_vec, &existing_alliance_ids)
            .await?;

        // From the fetched alliances, insert any missing factions to ID list
        for (_, alliance) in &fetched_alliances {
            if let Some(faction_id) = alliance.faction_id {
                faction_ids.insert(faction_id);
            }
        }

        // Fetch factions, if any missing ids
        let existing_faction_ids: Vec<i64> = faction_table_ids
            .iter()
            .map(|(_, faction_id)| *faction_id)
            .collect();
        let updated_factions = self
            .fetch_missing_factions(&faction_ids_vec, &existing_faction_ids)
            .await?;

        let updated_faction_ids: Vec<i64> = updated_factions.iter().map(|f| f.faction_id).collect();
        let affiliations = remove_invalid_faction_affiliations(
            affiliations,
            &updated_faction_ids,
            &faction_ids_vec,
            &existing_faction_ids,
        );

        // Insert all fetched entries to database

        // Update all affiliations

        Ok(())
    }

    async fn fetch_missing_corporations(
        &self,
        corporation_ids: &[i64],
        existing_corporation_ids: &[i64],
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        let missing_ids = get_missing_ids(corporation_ids, existing_corporation_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        CorporationService::new(&self.db, &self.esi_client)
            .get_many_corporations(missing_ids)
            .await
    }

    async fn fetch_missing_alliances(
        &self,
        alliance_ids: &[i64],
        existing_alliance_ids: &[i64],
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        let missing_ids = get_missing_ids(alliance_ids, existing_alliance_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        AllianceService::new(&self.db, &self.esi_client)
            .get_many_alliances(missing_ids)
            .await
    }

    /// Fetches and stores information for any factions missing from affiliations
    ///
    /// If a faction isn't found even after an update, then the affiliation entry for
    /// the character's faction will be set as none for the time being.
    async fn fetch_missing_factions(
        &self,
        faction_ids: &[i64],
        existing_faction_ids: &[i64],
    ) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let missing_ids = get_missing_ids(faction_ids, existing_faction_ids);

        if missing_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch any factions, alliances, & corporations which don't exist from ESI and insert them into database
        // This should rarely occur unless an update had just happened which added a new faction.
        // In which case we should be able to retrieve it from ESI if we haven't already updated factions
        // since downtime at 11:05 EVE time
        //
        // This returns an empty array if factions stored are still within 24 hour cache period.
        FactionService::new(&self.db, &self.esi_client)
            .update_factions()
            .await
    }
}

fn get_missing_ids(all_ids: &[i64], existing_ids: &[i64]) -> Vec<i64> {
    all_ids
        .iter()
        .filter(|id| !existing_ids.contains(id))
        .copied()
        .collect()
}

fn remove_invalid_faction_affiliations(
    mut affiliations: Vec<CharacterAffiliation>,
    updated_faction_ids: &[i64],
    faction_ids: &[i64],
    existing_faction_ids: &[i64],
) -> Vec<CharacterAffiliation> {
    let missing_ids = get_missing_ids(faction_ids, existing_faction_ids);
    let still_missing_faction_ids: Vec<i64> = missing_ids
        .into_iter()
        .filter(|id| !updated_faction_ids.contains(id))
        .collect();

    if still_missing_faction_ids.is_empty() {
        return affiliations;
    }

    // Set faction_id to None for affiliations with missing factions
    for affiliation in affiliations.iter_mut() {
        if let Some(faction_id) = affiliation.faction_id {
            if still_missing_faction_ids.contains(&faction_id) {
                tracing::warn!(
                    character_id = affiliation.character_id,
                    faction_id = faction_id,
                    "Character's faction ID could not be found in ESI; temporarily setting to None"
                );
                affiliation.faction_id = None;
            }
        }
    }

    return affiliations;
}

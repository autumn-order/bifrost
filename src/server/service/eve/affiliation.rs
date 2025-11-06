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
        let existing_faction_ids: HashSet<i64> =
            faction_table_ids.iter().map(|(_, id)| *id).collect();
        let missing_faction_ids: Vec<i64> = faction_ids
            .iter()
            .filter(|id| !existing_faction_ids.contains(id))
            .copied()
            .collect();

        // Insert all missing entries to database

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
    async fn resolve_faction_information(
        &self,
        mut affiliations: Vec<CharacterAffiliation>,
        faction_ids: HashSet<i64>,
    ) -> Result<Vec<CharacterAffiliation>, Error> {
        let faction_repo = FactionRepository::new(&self.db);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        // TEMP: This function will be refactored
        let missing_faction_ids = Vec::new();

        if missing_faction_ids.is_empty() {
            return Ok(affiliations);
        }

        // Fetch any factions, alliances, & corporations which don't exist from ESI and insert them into database
        // This should rarely occur unless an update had just happened which added a new faction.
        // In which case we should be able to retrieve it from ESI if we haven't already updated factions
        // since downtime at 11:05 EVE time
        //
        // This returns an empty array if factions stored are still within 24 hour cache period.
        let updated_factions = faction_service.update_factions().await?;

        // Check if update_factions returned the missing faction IDs
        let updated_faction_ids: Vec<i64> = updated_factions.iter().map(|f| f.faction_id).collect();
        let still_missing_faction_ids: Vec<i64> = missing_faction_ids
            .into_iter()
            .filter(|id| !updated_faction_ids.contains(id))
            .collect();

        if still_missing_faction_ids.is_empty() {
            return Ok(affiliations);
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

        Ok(affiliations)
    }
}

// Option 1: Extract the filtering logic as a helper function
fn get_missing_ids(all_ids: &[i64], existing_ids: &[i64]) -> Vec<i64> {
    all_ids
        .iter()
        .filter(|id| !existing_ids.contains(id))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use bifrost_test_utils::prelude::*;

    use super::*;

    mod resolve_faction_information {
        use chrono::{Duration, Utc};
        use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel};

        use crate::server::util::time::effective_faction_cache_expiry;

        use super::*;

        /// Expect Ok with unmodified affiliations when all factions exist in database
        #[tokio::test]
        async fn returns_affiliations_when_all_factions_exist() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction1 = test.eve().insert_mock_faction(1).await?;
            let faction2 = test.eve().insert_mock_faction(2).await?;
            let faction_endpoint = test.eve().with_faction_endpoint(1, 0);

            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 1001,
                    corporation_id: 2001,
                    alliance_id: Some(3001),
                    faction_id: Some(faction1.faction_id),
                },
                CharacterAffiliation {
                    character_id: 1002,
                    corporation_id: 2002,
                    alliance_id: Some(3002),
                    faction_id: Some(faction2.faction_id),
                },
            ];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 2);
            assert_eq!(
                returned_affiliations[0].faction_id,
                Some(faction1.faction_id)
            );
            assert_eq!(
                returned_affiliations[1].faction_id,
                Some(faction2.faction_id)
            );

            // Assert no request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with unmodified affiliations when no affiliations have faction_id
        #[tokio::test]
        async fn returns_affiliations_when_no_faction_ids() -> Result<(), TestError> {
            let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 1001,
                    corporation_id: 2001,
                    alliance_id: Some(3001),
                    faction_id: None,
                },
                CharacterAffiliation {
                    character_id: 1002,
                    corporation_id: 2002,
                    alliance_id: None,
                    faction_id: None,
                },
            ];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 2);
            assert_eq!(returned_affiliations[0].faction_id, None);
            assert_eq!(returned_affiliations[1].faction_id, None);

            Ok(())
        }

        /// Expect Ok with updated affiliations when missing factions can be fetched from ESI
        #[tokio::test]
        async fn fetches_missing_factions_from_esi() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction1 = test.eve().insert_mock_faction(1).await?;

            // Set faction last updated before today's faction update window to allow for updating
            // the faction from ESI
            let mut faction_model_am = entity::prelude::EveFaction::find_by_id(faction1.id)
                .one(&test.state.db)
                .await?
                .unwrap()
                .into_active_model();

            faction_model_am.updated_at =
                ActiveValue::Set((Utc::now() - Duration::hours(24)).naive_utc());

            entity::prelude::EveFaction::update(faction_model_am)
                .exec(&test.state.db)
                .await?;

            // Faction 2 is missing from database but will be available from ESI
            let faction_endpoint = test.eve().with_faction_endpoint(2, 1);

            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 1001,
                    corporation_id: 2001,
                    alliance_id: Some(3001),
                    faction_id: Some(faction1.faction_id),
                },
                CharacterAffiliation {
                    character_id: 1002,
                    corporation_id: 2002,
                    alliance_id: Some(3002),
                    faction_id: Some(2), // Missing faction
                },
            ];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 2);
            assert_eq!(
                returned_affiliations[0].faction_id,
                Some(faction1.faction_id)
            );
            assert_eq!(returned_affiliations[1].faction_id, Some(2));

            // Assert 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with faction_id set to None when missing faction cannot be fetched from ESI
        #[tokio::test]
        async fn sets_faction_id_to_none_when_not_in_esi() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            // ESI returns faction 1, but affiliations reference faction 2
            let faction_endpoint = test.eve().with_faction_endpoint(1, 1);

            let affiliations = vec![CharacterAffiliation {
                character_id: 1001,
                corporation_id: 2001,
                alliance_id: Some(3001),
                faction_id: Some(2), // This faction won't be in ESI response
            }];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 1);
            // faction_id should be set to None since it couldn't be found
            assert_eq!(returned_affiliations[0].faction_id, None);

            // Assert 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok without ESI call when missing factions are within cache period
        #[tokio::test]
        async fn skips_esi_when_factions_cached() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            let faction_endpoint = test.eve().with_faction_endpoint(2, 0);

            // Set updated_at to after effective expiry so it's considered cached
            let now = Utc::now();
            let effective_expiry = effective_faction_cache_expiry(now).unwrap();
            let updated_at = effective_expiry
                .checked_add_signed(Duration::minutes(1))
                .unwrap_or(effective_expiry);
            let mut faction_am = faction_model.into_active_model();
            faction_am.updated_at = ActiveValue::Set(updated_at);
            faction_am.update(&test.state.db).await?;

            let affiliations = vec![CharacterAffiliation {
                character_id: 1001,
                corporation_id: 2001,
                alliance_id: Some(3001),
                faction_id: Some(2), // Missing faction
            }];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 1);
            // faction_id should be set to None since update was skipped due to cache
            assert_eq!(returned_affiliations[0].faction_id, None);

            // Assert no request was made due to cache
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with mixed results: some factions exist, some fetched, some set to None
        #[tokio::test]
        async fn handles_mixed_faction_scenarios() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction1 = test.eve().insert_mock_faction(1).await?;

            // Set faction last updated before today's faction update window to allow for updating
            // the faction from ESI
            let mut faction_model_am = entity::prelude::EveFaction::find_by_id(faction1.id)
                .one(&test.state.db)
                .await?
                .unwrap()
                .into_active_model();

            faction_model_am.updated_at =
                ActiveValue::Set((Utc::now() - Duration::hours(24)).naive_utc());

            entity::prelude::EveFaction::update(faction_model_am)
                .exec(&test.state.db)
                .await?;

            // Faction 2 will be fetched from ESI, faction 3 won't be available
            let faction_endpoint = test.eve().with_faction_endpoint(2, 1);

            let affiliations = vec![
                CharacterAffiliation {
                    character_id: 1001,
                    corporation_id: 2001,
                    alliance_id: Some(3001),
                    faction_id: Some(faction1.faction_id), // Exists in DB
                },
                CharacterAffiliation {
                    character_id: 1002,
                    corporation_id: 2002,
                    alliance_id: Some(3002),
                    faction_id: Some(2), // Will be fetched from ESI
                },
                CharacterAffiliation {
                    character_id: 1003,
                    corporation_id: 2003,
                    alliance_id: None,
                    faction_id: Some(3), // Won't be found
                },
                CharacterAffiliation {
                    character_id: 1004,
                    corporation_id: 2004,
                    alliance_id: None,
                    faction_id: None, // No faction
                },
            ];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 4);
            assert_eq!(
                returned_affiliations[0].faction_id,
                Some(faction1.faction_id)
            ); // Existed
            assert_eq!(returned_affiliations[1].faction_id, Some(2)); // Fetched
            assert_eq!(returned_affiliations[2].faction_id, None); // Not found, set to None
            assert_eq!(returned_affiliations[3].faction_id, None); // Was already None

            // Assert 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Error when database connection fails
        #[tokio::test]
        async fn fails_when_database_unavailable() -> Result<(), TestError> {
            let test = test_setup_with_tables!()?;

            let affiliations = vec![CharacterAffiliation {
                character_id: 1001,
                corporation_id: 2001,
                alliance_id: Some(3001),
                faction_id: Some(1),
            }];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_err());

            Ok(())
        }

        /// Expect Ok with empty vector when input is empty
        #[tokio::test]
        async fn handles_empty_affiliations_list() -> Result<(), TestError> {
            let test = test_setup_with_tables!(entity::prelude::EveFaction)?;

            let affiliations: Vec<CharacterAffiliation> = vec![];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert!(returned_affiliations.is_empty());

            Ok(())
        }

        /// Expect Ok when update_factions is called but still doesn't find the faction (past cache expiry)
        #[tokio::test]
        async fn sets_to_none_after_update_past_cache() -> Result<(), TestError> {
            let mut test = test_setup_with_tables!(entity::prelude::EveFaction)?;
            let faction_model = test.eve().insert_mock_faction(1).await?;
            // ESI will return faction 10, but we're looking for faction 2
            let faction_endpoint = test.eve().with_faction_endpoint(10, 1);

            // Set updated_at to before effective expiry so update will be performed
            let now = Utc::now();
            let effective_expiry = effective_faction_cache_expiry(now).unwrap();
            let updated_at = effective_expiry
                .checked_sub_signed(Duration::minutes(5))
                .unwrap_or(effective_expiry);
            let mut faction_am = faction_model.into_active_model();
            faction_am.updated_at = ActiveValue::Set(updated_at);
            faction_am.update(&test.state.db).await?;

            let affiliations = vec![CharacterAffiliation {
                character_id: 1001,
                corporation_id: 2001,
                alliance_id: Some(3001),
                faction_id: Some(2), // Missing faction that won't be in ESI response
            }];

            let faction_ids: HashSet<i64> =
                affiliations.iter().filter_map(|a| a.faction_id).collect();

            let affiliation_service =
                AffiliationService::new(&test.state.db, &test.state.esi_client);
            let result = affiliation_service
                .resolve_faction_information(affiliations, faction_ids)
                .await;

            assert!(result.is_ok());
            let returned_affiliations = result.unwrap();
            assert_eq!(returned_affiliations.len(), 1);
            // faction_id should be set to None since it wasn't found even after update
            assert_eq!(returned_affiliations[0].faction_id, None);

            // Assert 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }
    }
}

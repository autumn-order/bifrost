use dioxus_logger::tracing;
use eve_esi::model::character::Character;
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::character::CharacterRepository,
    error::Error,
    service::orchestrator::{
        corporation::CorporationOrchestrator, faction::FactionOrchestrator, OrchestrationCache,
    },
};

const MAX_CONCURRENT_CHARACTER_FETCHES: usize = 10;

/// Orchestrator for fetching and persisting EVE characters and their dependencies
pub struct CharacterOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CharacterOrchestrator<'a> {
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Retrieve character entry ID from database that corresponds to provided EVE character ID
    pub async fn get_character_entry_id(
        &self,
        character_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_character_entry_ids(vec![character_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    /// Retrieve pairs of EVE character IDs & DB character IDs from a list of character IDs
    pub async fn get_many_character_entry_ids(
        &self,
        character_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = character_ids
            .iter()
            .filter(|id| !cache.character_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(character_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .character_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let character_repo = CharacterRepository::new(self.db);

        let retrieved_ids = character_repo
            .get_entry_ids_by_character_ids(&missing_ids)
            .await?;

        for (db_id, character_id) in retrieved_ids {
            cache.character_db_id.insert(character_id, db_id);
        }

        Ok(character_ids
            .iter()
            .filter_map(|id| {
                cache
                    .character_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    pub async fn fetch_character(
        &self,
        character_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Character, Error> {
        // Return character if it was already fetched and exists in cache
        if let Some(character) = cache.character_esi.get(&character_id) {
            return Ok(character.clone());
        }

        // Fetch character information from ESI
        let fetched_character = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .await?;

        // Insert the fetched character into cache to avoid additional ESI fetches on retries
        cache
            .character_esi
            .insert(character_id, fetched_character.clone());

        // Ensure the character's corporation exists in database else fetch it for persistence later
        let corporation_orch = CorporationOrchestrator::new(&self.db, &self.esi_client);
        corporation_orch
            .ensure_corporations_exist(vec![fetched_character.corporation_id], cache)
            .await?;

        // Ensure the character's faction exists in database else fetch it for persistence later
        if let Some(faction_id) = fetched_character.faction_id {
            let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);

            faction_orch
                .ensure_factions_exist(vec![faction_id], cache)
                .await?;
        }

        Ok(fetched_character)
    }

    pub async fn fetch_many_characters(
        &self,
        character_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, Character)>, Error> {
        // Check which IDs are missing from cache
        let missing_ids: Vec<i64> = character_ids
            .iter()
            .filter(|id| !cache.character_esi.contains_key(id))
            .copied()
            .collect();

        // If no IDs are missing, return cached characters
        if missing_ids.is_empty() {
            return Ok(character_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .character_esi
                        .get(id)
                        .map(|character| (*id, character.clone()))
                })
                .collect());
        }

        let mut fetched_characters = Vec::new();

        for chunk in missing_ids.chunks(MAX_CONCURRENT_CHARACTER_FETCHES) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let character = self
                        .esi_client
                        .character()
                        .get_character_public_information(id)
                        .await?;
                    Ok::<_, Error>((id, character))
                };
                futures.push(future);
            }

            while let Some(fetched_character) = futures.next().await {
                fetched_characters.push(fetched_character?);
            }
        }

        for (character_id, character) in &fetched_characters {
            cache.character_esi.insert(*character_id, character.clone());
        }

        let requested_characters: Vec<(i64, Character)> = character_ids
            .iter()
            .filter_map(|id| {
                cache
                    .character_esi
                    .get(id)
                    .map(|character| (*id, character.clone()))
            })
            .collect();

        let characters_ref: Vec<&Character> = requested_characters
            .iter()
            .map(|(_, character)| character)
            .collect();

        let faction_ids = cache.get_character_faction_dependency_ids(&characters_ref);
        let corporation_ids = cache.get_character_corporation_dependency_ids(&characters_ref);

        if !faction_ids.is_empty() {
            let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
            faction_orch
                .ensure_factions_exist(faction_ids, cache)
                .await?;
        }

        if !corporation_ids.is_empty() {
            let corporation_orch = CorporationOrchestrator::new(&self.db, &self.esi_client);
            corporation_orch
                .ensure_corporations_exist(corporation_ids, cache)
                .await?;
        }

        Ok(requested_characters)
    }

    pub async fn persist_characters(
        &self,
        txn: &DatabaseTransaction,
        characters: Vec<(i64, Character)>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_character::Model>, Error> {
        if characters.is_empty() {
            return Ok(Vec::new());
        }

        if cache.characters_persisted {
            return Ok(cache.character_model.values().cloned().collect());
        }

        // Persist factions if any were fetched
        let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
        faction_orch.persist_cached_factions(txn, cache).await?;

        // Persist corporations if any were fetched
        let corporation_orch = CorporationOrchestrator::new(&self.db, &self.esi_client);
        corporation_orch
            .persist_cached_corporations(txn, cache)
            .await?;

        let characters_ref: Vec<&Character> =
            characters.iter().map(|(_, character)| character).collect();

        let faction_ids = cache.get_character_faction_dependency_ids(&characters_ref);
        let corporation_ids = cache.get_character_corporation_dependency_ids(&characters_ref);

        let faction_db_ids = faction_orch
            .get_many_faction_entry_ids(faction_ids, cache)
            .await?;

        let corporation_db_ids = corporation_orch
            .get_many_corporation_entry_ids(corporation_ids, cache)
            .await?;

        // Create a map of faction/corporation id -> db_id for easy lookup
        let faction_id_map: std::collections::HashMap<i64, i32> =
            faction_db_ids.into_iter().collect();
        let corporation_id_map: std::collections::HashMap<i64, i32> =
            corporation_db_ids.into_iter().collect();

        // Map characters with their faction & corporation DB IDs
        let characters_to_upsert: Vec<(i64, Character, i32, Option<i32>)> = characters
            .into_iter()
            .filter_map(|(character_id, character)| {
                let faction_db_id = character.faction_id.and_then(|faction_id| {
                    let db_id = faction_id_map.get(&faction_id).copied();
                    if db_id.is_none() {
                        tracing::warn!(
                            "Failed to find faction ID {} for character ID {}; \
                            setting character's faction ID to None for now",
                            faction_id,
                            character_id
                        );
                    }
                    db_id
                });

                let corporation_db_id = corporation_id_map.get(&character.corporation_id).copied();
                if corporation_db_id.is_none() {
                    tracing::error!(
                        "Failed to find corporation ID {} for character ID {}; \
                        skipping character persistence",
                        character.corporation_id,
                        character_id
                    );
                    return None;
                }

                Some((
                    character_id,
                    character,
                    corporation_db_id.unwrap(),
                    faction_db_id,
                ))
            })
            .collect();

        // Upsert characters to database
        let character_repo = CharacterRepository::new(txn);
        let persisted_characters = character_repo.upsert_many(characters_to_upsert).await?;

        for model in &persisted_characters {
            cache
                .character_model
                .insert(model.character_id, model.clone());
        }

        cache.characters_persisted = true;

        Ok(persisted_characters)
    }
}

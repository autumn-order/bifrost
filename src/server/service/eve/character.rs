use eve_esi::model::character::Character;
use futures::future::join_all;
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::server::{
    data::eve::character::CharacterRepository,
    error::Error,
    service::{
        eve::{corporation::CorporationService, faction::FactionService},
        orchestrator::{character::CharacterOrchestrator, OrchestrationCache},
        retry::RetryContext,
    },
};

pub struct CharacterService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CharacterService<'a> {
    /// Creates a new instance of [`CharacterService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Updates information for provided character ID from ESI
    pub async fn update_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for character ID {}", character_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let character_orch = CharacterOrchestrator::new(&db, &esi_client);

                    let fetched_character =
                        character_orch.fetch_character(character_id, cache).await?;

                    // Reset persistence flags before transaction attempt in case of retry
                    cache.reset_persistence_flags();

                    let txn = db.begin().await?;

                    let model = character_orch
                        .persist(&txn, character_id, fetched_character, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(model)
                })
            },
        )
        .await
    }

    /// Fetches a character from EVE Online's ESI and creates a database entry
    pub async fn create_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let character_repo = CharacterRepository::new(self.db);
        let corporation_service = CorporationService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        let character = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .await?;

        let corporation_id = corporation_service
            .get_or_create_corporation(character.corporation_id)
            .await?
            .id;

        let faction_id = match character.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        let character = character_repo
            .create(character_id, character, corporation_id, faction_id)
            .await?;

        Ok(character)
    }

    /// Get character from database or create an entry for it from ESI
    pub async fn get_or_create_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let character_repo = CharacterRepository::new(self.db);

        if let Some(character) = character_repo.get_by_character_id(character_id).await? {
            return Ok(character);
        }

        let character = self.create_character(character_id).await?;

        Ok(character)
    }

    /// Fetches a list of characters from ESI using their character IDs
    /// Makes concurrent requests in batches of up to 10 at a time
    // TODO: unit tests, need to fix some bifrost-test-utils mock endpoint issues first
    pub async fn get_many_characters(
        &self,
        character_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Character)>, Error> {
        const BATCH_SIZE: usize = 10;
        let mut all_characters = Vec::new();

        // Process character IDs in chunks of BATCH_SIZE
        for chunk in character_ids.chunks(BATCH_SIZE) {
            // Create futures for all requests in this batch
            let futures: Vec<_> = chunk
                .iter()
                .map(|&character_id| async move {
                    let character = self
                        .esi_client
                        .character()
                        .get_character_public_information(character_id)
                        .await?;
                    Ok::<(i64, Character), Error>((character_id, character))
                })
                .collect();

            // Execute all futures in this batch concurrently
            let results = join_all(futures).await;

            // Collect results, propagating any errors
            for result in results {
                all_characters.push(result?);
            }
        }

        Ok(all_characters)
    }

    /// Fetches a character from EVE Online's ESI and upserts to database
    pub async fn upsert_character(
        &self,
        character_id: i64,
    ) -> Result<entity::eve_character::Model, Error> {
        let character_repo = CharacterRepository::new(self.db);
        let corporation_service = CorporationService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(&self.db, &self.esi_client);

        // Get character information from ESI
        let character = self
            .esi_client
            .character()
            .get_character_public_information(character_id)
            .await?;

        // Ensure corporation exists in database or create it if applicable to prevent foreign key error
        let corporation_id = corporation_service
            .get_or_create_corporation(character.corporation_id)
            .await?
            .id;

        // Ensure faction exists in database or create it if applicable to prevent foreign key error
        let faction_id = match character.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        // Update or create character in database
        let character = character_repo
            .upsert(character_id, character, corporation_id, faction_id)
            .await?;

        Ok(character)
    }
}

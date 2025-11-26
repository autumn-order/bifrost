use std::collections::HashSet;

use dioxus_logger::tracing;
use eve_esi::model::corporation::Corporation;
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::corporation::CorporationRepository,
    error::Error,
    service::orchestrator::{
        alliance::AllianceOrchestrator,
        cache::{get_corporation_alliance_dependency_ids, get_corporation_faction_dependency_ids},
        faction::FactionOrchestrator,
        OrchestrationCache,
    },
};

const MAX_CONCURRENT_CORPORATION_FETCHES: usize = 10;

pub struct CorporationOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CorporationOrchestrator<'a> {
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn get_corporation_entry_id(
        &self,
        corporation_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Option<i32>, Error> {
        let ids = self
            .get_many_corporation_entry_ids(vec![corporation_id], cache)
            .await?;

        Ok(ids.into_iter().next().map(|(_, db_id)| db_id))
    }

    pub async fn get_many_corporation_entry_ids(
        &self,
        corporation_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, i32)>, Error> {
        let missing_ids: Vec<i64> = corporation_ids
            .iter()
            .filter(|id| !cache.corporation_db_id.contains_key(id))
            .copied()
            .collect();

        if missing_ids.is_empty() {
            return Ok(corporation_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .corporation_db_id
                        .get(id)
                        .cloned()
                        .map(|db_id| (*id, db_id))
                })
                .collect());
        }

        let corporation_repo = CorporationRepository::new(self.db);

        let retrieved_ids = corporation_repo
            .get_entry_ids_by_corporation_ids(&missing_ids)
            .await?;

        for (db_id, corporation_id) in retrieved_ids {
            cache.corporation_db_id.insert(corporation_id, db_id);
        }

        Ok(corporation_ids
            .iter()
            .filter_map(|id| {
                cache
                    .corporation_db_id
                    .get(id)
                    .cloned()
                    .map(|db_id| (*id, db_id))
            })
            .collect())
    }

    pub async fn fetch_corporation(
        &self,
        corporation_id: i64,
        cache: &mut OrchestrationCache,
    ) -> Result<Corporation, Error> {
        // Return corporation if it was already fetched and exists in cache
        if let Some(corporation) = cache.corporation_esi.get(&corporation_id) {
            return Ok(corporation.clone());
        }

        // Fetch corporation information from ESI
        let fetched_corporation = self
            .esi_client
            .corporation()
            .get_corporation_information(corporation_id)
            .await?;

        // Insert the fetched corporation into cache to avoid additional ESI fetches on retries
        cache
            .corporation_esi
            .insert(corporation_id, fetched_corporation.clone());

        // Ensure the corporation's alliance exists in database else fetch it for persistence later
        if let Some(alliance_id) = fetched_corporation.alliance_id {
            let alliance_orch = AllianceOrchestrator::new(&self.db, &self.esi_client);

            alliance_orch
                .ensure_alliances_exist(vec![alliance_id], cache)
                .await?;
        }

        // Ensure the corporation's faction exists in database else fetch it for persistence later
        if let Some(faction_id) = fetched_corporation.faction_id {
            let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);

            faction_orch
                .ensure_factions_exist(vec![faction_id], cache)
                .await?;
        }

        Ok(fetched_corporation)
    }

    pub async fn fetch_many_corporations(
        &self,
        corporation_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        // Check which IDs are missing from cache
        let missing_ids: Vec<i64> = corporation_ids
            .iter()
            .filter(|id| !cache.corporation_esi.contains_key(id))
            .copied()
            .collect();

        // If no IDs are missing, return cached corporations
        if missing_ids.is_empty() {
            return Ok(corporation_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .corporation_esi
                        .get(id)
                        .map(|corporation| (*id, corporation.clone()))
                })
                .collect());
        }

        let mut fetched_corporations = Vec::new();

        for chunk in missing_ids.chunks(MAX_CONCURRENT_CORPORATION_FETCHES) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let corporation = self
                        .esi_client
                        .corporation()
                        .get_corporation_information(id)
                        .await?;
                    Ok::<_, Error>((id, corporation))
                };
                futures.push(future);
            }

            while let Some(fetched_corporation) = futures.next().await {
                fetched_corporations.push(fetched_corporation?);
            }
        }

        for (corporation_id, corporation) in &fetched_corporations {
            cache
                .corporation_esi
                .insert(*corporation_id, corporation.clone());
        }

        let requested_corporations: Vec<(i64, Corporation)> = corporation_ids
            .iter()
            .filter_map(|id| {
                cache
                    .corporation_esi
                    .get(id)
                    .map(|corporation| (*id, corporation.clone()))
            })
            .collect();

        let corporations_ref: Vec<&Corporation> = requested_corporations
            .iter()
            .map(|(_, corporation)| corporation)
            .collect();

        let faction_ids = get_corporation_faction_dependency_ids(&corporations_ref);
        let alliance_ids = get_corporation_alliance_dependency_ids(&corporations_ref);

        if !faction_ids.is_empty() {
            let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
            faction_orch
                .ensure_factions_exist(faction_ids, cache)
                .await?;
        }

        if !alliance_ids.is_empty() {
            let alliance_orch = AllianceOrchestrator::new(&self.db, &self.esi_client);
            alliance_orch
                .ensure_alliances_exist(alliance_ids, cache)
                .await?;
        }

        Ok(requested_corporations)
    }

    pub async fn persist_corporations(
        &self,
        txn: &DatabaseTransaction,
        corporations: Vec<(i64, Corporation)>,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_corporation::Model>, Error> {
        if corporations.is_empty() {
            return Ok(Vec::new());
        }

        if cache.corporations_persisted {
            return Ok(cache.corporation_model.values().cloned().collect());
        }

        // Persist factions if any were fetched
        let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
        faction_orch.persist_cached_factions(txn, cache).await?;

        // Persist alliances if any were fetched
        let alliance_orch = AllianceOrchestrator::new(&self.db, &self.esi_client);
        alliance_orch.persist_cached_alliances(txn, cache).await?;

        let corporations_ref: Vec<&Corporation> = corporations
            .iter()
            .map(|(_, corporation)| corporation)
            .collect();

        let faction_ids = get_corporation_faction_dependency_ids(&corporations_ref);
        let alliance_ids = get_corporation_alliance_dependency_ids(&corporations_ref);

        let faction_db_ids = faction_orch
            .get_many_faction_entry_ids(faction_ids, cache)
            .await?;

        let alliance_db_ids = alliance_orch
            .get_many_alliance_entry_ids(alliance_ids, cache)
            .await?;

        // Create a map of faction/alliance id -> db_id for easy lookup
        let faction_id_map: std::collections::HashMap<i64, i32> =
            faction_db_ids.into_iter().collect();
        let alliance_id_map: std::collections::HashMap<i64, i32> =
            alliance_db_ids.into_iter().collect();

        // Map corporations with their faction & alliance DB IDs
        let corporations_to_upsert: Vec<(i64, Corporation, Option<i32>, Option<i32>)> =
            corporations
                .into_iter()
                .map(|(corporation_id, corporation)| {
                    let faction_db_id = corporation.faction_id.and_then(|faction_id| {
                        let db_id = faction_id_map.get(&faction_id).copied();
                        if db_id.is_none() {
                            tracing::warn!(
                                "Failed to find faction ID {} for corporation ID {}; \
                                setting corporation's faction ID to None for now",
                                faction_id,
                                corporation_id
                            )
                        }
                        db_id
                    });

                    let alliance_db_id = corporation.alliance_id.and_then(|alliance_id| {
                        let db_id = alliance_id_map.get(&alliance_id).copied();
                        if db_id.is_none() {
                            tracing::warn!(
                                "Failed to find alliance ID {} for corporation ID {}; \
                                setting corporation's alliance ID to None for now",
                                alliance_id,
                                corporation_id
                            )
                        }
                        db_id
                    });

                    (corporation_id, corporation, alliance_db_id, faction_db_id)
                })
                .collect();

        // Upsert corporations to database
        let corporation_repo = CorporationRepository::new(txn);
        let persisted_corporations = corporation_repo.upsert_many(corporations_to_upsert).await?;

        for model in &persisted_corporations {
            cache
                .corporation_model
                .insert(model.corporation_id, model.clone());
            cache
                .corporation_db_id
                .insert(model.corporation_id, model.id);
        }

        cache.corporations_persisted = true;

        Ok(persisted_corporations)
    }

    /// Check database for provided corporation ids, fetch corporations from ESI if any are missing
    pub(super) async fn ensure_corporations_exist(
        &self,
        corporation_ids: Vec<i64>,
        cache: &mut OrchestrationCache,
    ) -> Result<(), Error> {
        let existing_ids = self
            .get_many_corporation_entry_ids(corporation_ids.clone(), cache)
            .await?;

        let existing_corporation_ids: HashSet<i64> =
            existing_ids.iter().map(|(id, _)| *id).collect();

        let missing_ids: Vec<i64> = corporation_ids
            .into_iter()
            .filter(|id| !existing_corporation_ids.contains(id))
            .collect();

        if missing_ids.is_empty() {
            return Ok(());
        }

        // Fetch the corporations if any IDs are missing
        self.fetch_many_corporations(missing_ids, cache).await?;

        Ok(())
    }

    /// Persist any corporations currently in the ESI cache
    pub(super) async fn persist_cached_corporations(
        &self,
        txn: &DatabaseTransaction,
        cache: &mut OrchestrationCache,
    ) -> Result<Vec<entity::eve_corporation::Model>, Error> {
        let corporations: Vec<(i64, Corporation)> = cache
            .corporation_esi
            .iter()
            .map(|(id, corporation)| (*id, corporation.clone()))
            .collect();
        self.persist_corporations(txn, corporations, cache).await
    }
}

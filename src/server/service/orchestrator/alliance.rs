use std::collections::{HashMap, HashSet};

use dioxus_logger::tracing;
use eve_esi::model::alliance::Alliance;
use futures::stream::{FuturesUnordered, StreamExt};
use sea_orm::{DatabaseConnection, DatabaseTransaction};

use crate::server::{
    data::eve::alliance::AllianceRepository,
    error::{eve::EveError, Error},
    service::orchestrator::faction::{FactionOrchestrationCache, FactionOrchestrator},
};

static MAX_CONCURRENT_ALLIANCE_FETCHES: usize = 10;

#[derive(Clone, Default, Debug)]
pub struct AllianceOrchestrationCache {
    pub alliance_esi: HashMap<i64, Alliance>,
    pub alliance_model: HashMap<i64, entity::eve_alliance::Model>,
    pub faction: FactionOrchestrationCache,
    already_persisted: bool,
}

pub struct AllianceOrchestrator<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AllianceOrchestrator<'a> {
    pub async fn fetch_alliance(
        &self,
        alliance_id: i64,
        cache: &mut AllianceOrchestrationCache,
    ) -> Result<Alliance, Error> {
        // Return alliance if it was already fetched and exists in cache
        if let Some(alliance) = cache.alliance_esi.get(&alliance_id) {
            return Ok(alliance.clone());
        }

        // Fetch alliance information from ESI
        let fetched_alliance = self
            .esi_client
            .alliance()
            .get_alliance_information(alliance_id)
            .await?;

        // Insert the fetch alliance into cache to avoid additional ESI fetches on retries
        cache
            .alliance_esi
            .insert(alliance_id, fetched_alliance.clone());

        // If there is no faction ID, return the alliance
        let Some(faction_id) = fetched_alliance.faction_id else {
            return Ok(fetched_alliance);
        };

        // Ensure the faction ID for the alliance exists in database, else the faction
        // orchestrator will attempt to fetch updated factions from ESI if the factions
        // we have currently stored are out of date
        let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
        faction_orch
            .ensure_factions_exist(vec![faction_id], &mut cache.faction)
            .await?;

        Ok(fetched_alliance)
    }

    pub async fn fetch_many_alliances(
        &self,
        alliance_ids: Vec<i64>,
        cache: &mut AllianceOrchestrationCache,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        // Check which IDs are missing from cache
        let missing_ids: Vec<i64> = alliance_ids
            .iter()
            .filter(|id| !cache.alliance_esi.contains_key(id))
            .copied()
            .collect();

        // If no IDs are missing, return cached alliances
        if missing_ids.is_empty() {
            return Ok(alliance_ids
                .iter()
                .filter_map(|id| {
                    cache
                        .alliance_esi
                        .get(id)
                        .map(|alliance| (*id, alliance.clone()))
                })
                .collect());
        }

        let mut fetched_alliances = Vec::new();

        for chunk in missing_ids.chunks(MAX_CONCURRENT_ALLIANCE_FETCHES) {
            let mut futures = FuturesUnordered::new();

            for &id in chunk {
                let future = async move {
                    let alliance = self
                        .esi_client
                        .alliance()
                        .get_alliance_information(id)
                        .await?;
                    Ok::<_, Error>((id, alliance))
                };
                futures.push(future);
            }

            while let Some(fetched_alliance) = futures.next().await {
                fetched_alliances.push(fetched_alliance?);
            }
        }

        for (alliance_id, alliance) in &fetched_alliances {
            cache.alliance_esi.insert(*alliance_id, alliance.clone());
        }

        let requested_alliances: Vec<(i64, Alliance)> = alliance_ids
            .iter()
            .filter_map(|id| {
                cache
                    .alliance_esi
                    .get(id)
                    .map(|alliance| (*id, alliance.clone()))
            })
            .collect();

        let faction_ids: HashSet<i64> = fetched_alliances
            .iter()
            .filter_map(|(_, alliance)| alliance.faction_id)
            .collect();

        if faction_ids.is_empty() {
            return Ok(requested_alliances);
        }

        // Ensure the faction IDs for the alliances exists in the database, else the faction
        // orchestrator will attempt to fetch updated factions from ESI if the factions
        // we have currently stored are out of date
        let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
        faction_orch
            .ensure_factions_exist(faction_ids.into_iter().collect(), &mut cache.faction)
            .await?;

        Ok(requested_alliances)
    }

    pub async fn persist_alliances(
        &self,
        txn: &DatabaseTransaction,
        alliances: Vec<(i64, Alliance)>,
        cache: &mut AllianceOrchestrationCache,
    ) -> Result<Vec<entity::eve_alliance::Model>, Error> {
        if alliances.is_empty() {
            return Ok(Vec::new());
        }

        if cache.already_persisted {
            return Ok(cache.alliance_model.values().cloned().collect());
        };

        // Persist factions if any were fetched
        let faction_orch = FactionOrchestrator::new(&self.db, &self.esi_client);
        faction_orch
            .persist_cached_factions(txn, &mut cache.faction)
            .await?;

        // Get the DB IDs for factions to map to alliances
        let faction_ids: Vec<i64> = alliances
            .iter()
            .filter_map(|(_, alliance)| alliance.faction_id)
            .collect();

        let faction_db_ids = faction_orch
            .get_faction_entry_ids(faction_ids, &mut cache.faction)
            .await?;

        // Create a map of faction_id -> db_id for easy lookup
        let faction_id_map: HashMap<i64, i32> = faction_db_ids.into_iter().collect();

        // Map alliances with their faction DB IDs
        let alliances_to_upsert: Vec<(i64, Alliance, Option<i32>)> = alliances
            .into_iter()
            .map(|(alliance_id, alliance)| {
                let faction_db_id = alliance.faction_id.and_then(|faction_id| {
                    let db_id = faction_id_map.get(&faction_id).copied();
                    if db_id.is_none() {
                        tracing::warn!("{}", EveError::FactionNotFound(faction_id));
                    }
                    db_id
                });
                (alliance_id, alliance, faction_db_id)
            })
            .collect();

        // Upsert alliances to database
        let alliance_repo = AllianceRepository::new(txn);
        let persisted_alliances = alliance_repo.upsert_many(alliances_to_upsert).await?;

        for model in &persisted_alliances {
            cache
                .alliance_model
                .insert(model.alliance_id, model.clone());
        }

        cache.already_persisted = true;

        Ok(persisted_alliances)
    }
}

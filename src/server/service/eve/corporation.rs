use eve_esi::model::corporation::Corporation;
use futures::future::join_all;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::corporation::CorporationRepository,
    error::Error,
    service::eve::{alliance::AllianceService, faction::FactionService},
};

pub struct CorporationService {
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
}

impl CorporationService {
    /// Creates a new instance of [`CorporationService`]
    pub fn new(db: DatabaseConnection, esi_client: eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches a corporation from EVE Online's ESI and creates a database entry
    pub async fn create_corporation(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let corporation_repo = CorporationRepository::new(&self.db);
        let alliance_service = AllianceService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(self.db.clone(), self.esi_client.clone());

        let corporation = self
            .esi_client
            .corporation()
            .get_corporation_information(corporation_id)
            .await?;

        let alliance_id = match corporation.alliance_id {
            Some(id) => Some(alliance_service.get_or_create_alliance(id).await?.id),
            None => None,
        };

        let faction_id = match corporation.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        let corporation = corporation_repo
            .create(corporation_id, corporation, alliance_id, faction_id)
            .await?;

        Ok(corporation)
    }

    /// Fetches a list of corporations from ESI using their corporation IDs
    /// Makes concurrent requests in batches of up to 10 at a time
    pub async fn get_many_corporations(
        &self,
        corporation_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Corporation)>, Error> {
        const BATCH_SIZE: usize = 10;
        let mut all_corporations = Vec::new();

        // Process corporation IDs in chunks of BATCH_SIZE
        for chunk in corporation_ids.chunks(BATCH_SIZE) {
            // Create futures for all requests in this batch
            let futures: Vec<_> = chunk
                .iter()
                .map(|&corporation_id| async move {
                    let corporation = self
                        .esi_client
                        .corporation()
                        .get_corporation_information(corporation_id)
                        .await?;
                    Ok::<(i64, Corporation), Error>((corporation_id, corporation))
                })
                .collect();

            // Execute all futures in this batch concurrently
            let results = join_all(futures).await;

            // Collect results, propagating any errors
            for result in results {
                all_corporations.push(result?);
            }
        }

        Ok(all_corporations)
    }

    /// Get corporation from database or create an entry for it from ESI
    pub async fn get_or_create_corporation(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let corporation_repo = CorporationRepository::new(&self.db);

        if let Some(corporation) = corporation_repo
            .get_by_corporation_id(corporation_id)
            .await?
        {
            return Ok(corporation);
        }

        let corporation = self.create_corporation(corporation_id).await?;

        Ok(corporation)
    }

    /// Fetches a corporation from EVE Online's ESI and updates or creates a database entry
    pub async fn upsert_corporation(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let corporation_repo = CorporationRepository::new(&self.db);
        let alliance_service = AllianceService::new(&self.db, &self.esi_client);
        let faction_service = FactionService::new(self.db.clone(), self.esi_client.clone());

        // Get corporation information from ESI
        let corporation = self
            .esi_client
            .corporation()
            .get_corporation_information(corporation_id)
            .await?;

        // Ensure alliance exists in database or create it if applicable to prevent foreign key error
        let alliance_id = match corporation.alliance_id {
            Some(alliance_id) => Some(
                alliance_service
                    .get_or_create_alliance(alliance_id)
                    .await?
                    .id,
            ),
            None => None,
        };

        // Ensure faction exists in database or create it if applicable to prevent foreign key error
        let faction_id = match corporation.faction_id {
            Some(faction_id) => Some(faction_service.get_or_update_factions(faction_id).await?.id),
            None => None,
        };

        // Update or create corporation in database
        let corporation = corporation_repo
            .upsert(corporation_id, corporation, alliance_id, faction_id)
            .await?;

        Ok(corporation)
    }
}

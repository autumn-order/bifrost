use eve_esi::model::alliance::Alliance;
use futures::future::join_all;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::alliance::AllianceRepository, error::Error, service::eve::faction::FactionService,
};

pub struct AllianceService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> AllianceService<'a> {
    /// Creates a new instance of [`AllianceService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches an alliance from EVE Online's ESI and creates a database entry
    pub async fn create_alliance(
        &self,
        alliance_id: i64,
    ) -> Result<entity::eve_alliance::Model, Error> {
        let alliance_repo = AllianceRepository::new(&self.db);
        let faction_service = FactionService::new(self.db.clone(), self.esi_client.clone());

        let alliance = self
            .esi_client
            .alliance()
            .get_alliance_information(alliance_id)
            .await?;

        let faction_id = match alliance.faction_id {
            Some(id) => Some(faction_service.get_or_update_factions(id).await?.id),
            None => None,
        };

        let alliance = alliance_repo
            .create(alliance_id, alliance, faction_id)
            .await?;

        Ok(alliance)
    }

    /// Fetches a list of alliances from ESI using their alliance IDs
    /// Makes concurrent requests in batches of up to 10 at a time
    pub async fn get_many_alliances(
        &self,
        alliance_ids: Vec<i64>,
    ) -> Result<Vec<(i64, Alliance)>, Error> {
        const BATCH_SIZE: usize = 10;
        let mut all_alliances = Vec::new();

        for chunk in alliance_ids.chunks(BATCH_SIZE) {
            let futures: Vec<_> = chunk
                .iter()
                .map(|&alliance_id| async move {
                    let alliance = self
                        .esi_client
                        .alliance()
                        .get_alliance_information(alliance_id)
                        .await?;
                    Ok::<(i64, Alliance), Error>((alliance_id, alliance))
                })
                .collect();

            let results = join_all(futures).await;

            for result in results {
                all_alliances.push(result?);
            }
        }

        Ok(all_alliances)
    }

    /// Gets an alliance from database or creates an entry for it from ESI
    pub async fn get_or_create_alliance(
        &self,
        alliance_id: i64,
    ) -> Result<entity::eve_alliance::Model, Error> {
        let alliance_repo = AllianceRepository::new(&self.db);

        if let Some(alliance) = alliance_repo.get_by_alliance_id(alliance_id).await? {
            return Ok(alliance);
        }

        let alliance = self.create_alliance(alliance_id).await?;

        Ok(alliance)
    }

    /// Updates or creates an entry for provided alliance ID
    pub async fn upsert_alliance(
        &self,
        alliance_id: i64,
    ) -> Result<entity::eve_alliance::Model, Error> {
        let alliance_repo = AllianceRepository::new(&self.db);
        let faction_service = FactionService::new(self.db.clone(), self.esi_client.clone());

        // Get alliance information from ESI
        let alliance = self
            .esi_client
            .alliance()
            .get_alliance_information(alliance_id)
            .await?;

        // Ensure faction exists in database or create it if applicable to prevent foreign key error
        let faction_id = match alliance.faction_id {
            Some(faction_id) => Some(faction_service.get_or_update_factions(faction_id).await?.id),
            None => None,
        };

        // Update or create alliance in database
        let alliance = alliance_repo
            .upsert(alliance_id, alliance, faction_id)
            .await?;

        Ok(alliance)
    }
}

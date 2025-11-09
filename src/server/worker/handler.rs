use dioxus_logger::tracing;
use sea_orm::{DatabaseConnection, EntityTrait, QuerySelect};

use crate::server::{
    error::Error,
    service::eve::{
        affiliation::AffiliationService, alliance::AllianceService, character::CharacterService,
        corporation::CorporationService,
    },
};

pub struct WorkerJobHandler<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> WorkerJobHandler<'a> {
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    pub async fn update_alliance_info(&self, alliance_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing alliance info update for alliance_id: {}",
            alliance_id
        );

        AllianceService::new(&self.db, &self.esi_client)
            .upsert_alliance(alliance_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update info for alliance {}: {:?}",
                    alliance_id,
                    e
                );
                e
            })?;

        tracing::debug!("Successfully updated info for alliance {}", alliance_id);

        Ok(())
    }

    pub async fn update_corporation_info(&self, corporation_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing corporation info update for corporation_id: {}",
            corporation_id
        );

        CorporationService::new(&self.db, &self.esi_client)
            .upsert_corporation(corporation_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update info for corporation {}: {:?}",
                    corporation_id,
                    e
                );
                e
            })?;

        tracing::debug!(
            "Successfully updated info for corporation {}",
            corporation_id
        );

        Ok(())
    }

    pub async fn update_character_info(&self, character_id: i64) -> Result<(), Error> {
        tracing::debug!(
            "Processing character info update for character_id: {}",
            character_id
        );

        CharacterService::new(&self.db, &self.esi_client)
            .upsert_character(character_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update info for character {}: {:?}",
                    character_id,
                    e
                );
                e
            })?;

        tracing::debug!("Successfully updated info for character {}", character_id);

        Ok(())
    }

    /// Retrieves list of character IDs equal to provided count and updates affiliations for all of them
    pub async fn update_affiliations(&self, count: u64) -> Result<(), Error> {
        tracing::debug!("Processing affiliations update for {} characters", count);

        let character_ids: Vec<i64> = entity::prelude::EveCharacter::find()
            .select_only()
            .column(entity::eve_character::Column::CharacterId)
            .limit(count)
            .into_tuple()
            .all(self.db)
            .await.map_err(|e| {
                tracing::error!("Failed to retrieve character IDs for an affiliations update due to error: {:?}", e);
                e
            })?;

        AffiliationService::new(self.db, self.esi_client)
            .update_affiliations(character_ids)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update affiliations due to error: {:?}", e);
                e
            })?;

        tracing::debug!("Successfully updated affiliations for {} characters", count);

        Ok(())
    }
}

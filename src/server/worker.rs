use apalis::prelude::Data;
use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    model::worker::WorkerJob,
    service::eve::{
        alliance::AllianceService, character::CharacterService, corporation::CorporationService,
    },
};

pub struct WorkerJobHandler<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

pub async fn handle_job(
    job: WorkerJob,
    db: Data<DatabaseConnection>,
    esi_client: Data<eve_esi::Client>,
) -> Result<(), Error> {
    let handler = WorkerJobHandler::new(&db, &esi_client);

    match job {
        WorkerJob::UpdateAllianceInfo { alliance_id } => {
            handler.update_alliance_info(alliance_id).await?
        }
        WorkerJob::UpdateCorporationInfo { corporation_id } => {
            handler.update_corporation_info(corporation_id).await?
        }
        WorkerJob::UpdateCharacterInfo { character_id } => {
            handler.update_character_info(character_id).await?
        }
    }

    Ok(())
}

impl<'a> WorkerJobHandler<'a> {
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    async fn update_alliance_info(&self, alliance_id: i64) -> Result<(), Error> {
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

    async fn update_corporation_info(&self, corporation_id: i64) -> Result<(), Error> {
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

    async fn update_character_info(&self, character_id: i64) -> Result<(), Error> {
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
}

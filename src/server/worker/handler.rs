use dioxus_logger::tracing;
use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    model::worker::WorkerJob,
    service::eve::{
        affiliation::AffiliationService, alliance::AllianceService, character::CharacterService,
        corporation::CorporationService,
    },
    util::eve::ESI_AFFILIATION_REQUEST_LIMIT,
};

/// Handler for processing worker jobs from the queue
///
/// This handler provides a centralized interface for executing different types
/// of worker jobs. Each job type has a corresponding method that handles the
/// specific business logic.
pub struct WorkerJobHandler {
    db: DatabaseConnection,
    esi_client: eve_esi::Client,
}

impl WorkerJobHandler {
    /// Create a new WorkerJobHandler
    pub fn new(db: DatabaseConnection, esi_client: eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Handle a worker job by delegating to the appropriate handler method
    ///
    /// This is the main entry point for job processing. It dispatches the job
    /// to the correct handler method based on the job type.
    pub async fn handle(&self, job: &WorkerJob) -> Result<(), Error> {
        match job {
            WorkerJob::UpdateAllianceInfo { alliance_id } => {
                self.update_alliance_info(*alliance_id).await
            }
            WorkerJob::UpdateCorporationInfo { corporation_id } => {
                self.update_corporation_info(*corporation_id).await
            }
            WorkerJob::UpdateCharacterInfo { character_id } => {
                self.update_character_info(*character_id).await
            }
            WorkerJob::UpdateAffiliations { character_ids } => {
                self.update_affiliations(character_ids.clone()).await
            }
        }
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

        CorporationService::new(self.db.clone(), self.esi_client.clone())
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

    /// Updates affiliations for the provided list of character IDs
    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), Error> {
        let count = character_ids.len();
        tracing::debug!("Processing affiliations update for {} characters", count);

        if character_ids.is_empty() {
            tracing::debug!("No characters to update affiliations for");
            return Ok(());
        }

        if character_ids.len() > ESI_AFFILIATION_REQUEST_LIMIT {
            tracing::warn!(
                "Update affiliation job contains {} character IDs, exceeding ESI affiliation request limit of {}; truncating to limit",
                character_ids.len(),
                ESI_AFFILIATION_REQUEST_LIMIT
            );
        }

        AffiliationService::new(&self.db, &self.esi_client)
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

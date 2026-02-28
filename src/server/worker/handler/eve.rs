use dioxus_logger::tracing;

use super::WorkerJobHandler;
use crate::server::{
    error::AppError,
    service::eve::{
        affiliation::AffiliationService, alliance::AllianceService, character::CharacterService,
        corporation::CorporationService, faction::FactionService,
    },
    util::eve::ESI_AFFILIATION_REQUEST_LIMIT,
};

impl WorkerJobHandler {
    /// Updates NPC faction information from ESI.
    ///
    /// Checks if the faction cache has expired and fetches updated faction data from ESI
    /// if needed. ESI caches faction data for 24 hours, so this may not fetch new data
    /// on every call.
    ///
    /// # Returns
    /// - `Ok(())` - Faction update completed (or skipped if cache valid)
    /// - `Err(AppError)` - Failed to update factions
    pub async fn update_faction_info(&self) -> Result<(), AppError> {
        tracing::debug!("Checking for daily NPC faction info update");

        let factions = FactionService::new(&self.db, &self.esi_provider)
            .update()
            .await
            .map_err(|e| {
                tracing::error!("Failed to update NPC faction information: {:?}", e);
                e
            })?;

        if factions.is_empty() {
            tracing::debug!("NPC faction information already up to date, no update needed");
        } else {
            tracing::debug!(
                "Successfully updated NPC faction information for {} factions",
                factions.len()
            );
        }

        Ok(())
    }

    /// Updates alliance information from ESI.
    ///
    /// Fetches alliance data from ESI and persists it to the database. If the alliance
    /// has faction affiliations, those dependencies are resolved first.
    ///
    /// # Arguments
    /// - `alliance_id` - EVE Online alliance ID to update
    ///
    /// # Returns
    /// - `Ok(())` - Alliance info updated successfully
    /// - `Err(AppError)` - Failed to fetch or persist alliance data
    pub async fn update_alliance_info(&self, alliance_id: i64) -> Result<(), AppError> {
        tracing::debug!(
            "Processing alliance info update for alliance_id: {}",
            alliance_id
        );

        AllianceService::new(&self.db, &self.esi_provider)
            .update(alliance_id)
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

    /// Updates corporation information from ESI.
    ///
    /// Fetches corporation data from ESI and persists it to the database. If the corporation
    /// has alliance or faction affiliations, those dependencies are resolved first.
    ///
    /// # Arguments
    /// - `corporation_id` - EVE Online corporation ID to update
    ///
    /// # Returns
    /// - `Ok(())` - Corporation info updated successfully
    /// - `Err(AppError)` - Failed to fetch or persist corporation data
    pub async fn update_corporation_info(&self, corporation_id: i64) -> Result<(), AppError> {
        tracing::debug!(
            "Processing corporation info update for corporation_id: {}",
            corporation_id
        );

        CorporationService::new(&self.db, &self.esi_provider)
            .update(corporation_id)
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

    /// Updates character information from ESI.
    ///
    /// Fetches character data from ESI and persists it to the database. If the character
    /// has corporation or faction affiliations, those dependencies are resolved first.
    ///
    /// # Arguments
    /// - `character_id` - EVE Online character ID to update
    ///
    /// # Returns
    /// - `Ok(())` - Character info updated successfully
    /// - `Err(AppError)` - Failed to fetch or persist character data
    pub async fn update_character_info(&self, character_id: i64) -> Result<(), AppError> {
        tracing::debug!(
            "Processing character info update for character_id: {}",
            character_id
        );

        CharacterService::new(&self.db, &self.esi_provider)
            .update(character_id)
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

    /// Updates affiliations for multiple characters in bulk.
    ///
    /// Fetches character affiliation data from ESI and updates both character-to-corporation
    /// and corporation-to-alliance relationships. Validates the character ID list and
    /// truncates to ESI's limit of 1000 characters if necessary.
    ///
    /// # Arguments
    /// - `character_ids` - List of EVE Online character IDs to update affiliations for
    ///
    /// # Returns
    /// - `Ok(())` - Affiliations updated successfully
    /// - `Err(AppError)` - Failed to fetch or persist affiliation data
    pub async fn update_affiliations(&self, character_ids: Vec<i64>) -> Result<(), AppError> {
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

        AffiliationService::new(&self.db, &self.esi_provider)
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

use chrono::Utc;
use sea_orm::DatabaseConnection;

use crate::server::{
    data::eve::faction::FactionRepository, error::Error, util::time::effective_faction_cache_expiry,
};

pub struct FactionService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> FactionService<'a> {
    /// Creates a new instance of [`FactionService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Fetches & stores NPC faction information from ESI so long as they aren't within cache period
    ///
    /// The NPC faction cache expires at 11:05 UTC (after downtime)
    pub async fn update_factions(&self) -> Result<Vec<entity::eve_faction::Model>, Error> {
        let faction_repo = FactionRepository::new(self.db);

        let now = Utc::now();
        let effective_expiry = effective_faction_cache_expiry(now)?;

        // If the latest faction entry was updated at or after the effective expiry, skip updating.
        if let Some(faction) = faction_repo.get_latest().await? {
            if faction.updated_at >= effective_expiry {
                return Ok(Vec::new());
            }
        }

        let factions = self.esi_client.universe().get_factions().await?;

        let factions = faction_repo.upsert_many(factions).await?;

        Ok(factions)
    }

    /// Attempt to get a faction from the database using its EVE Online faction ID, attempt to update factions if faction is not found
    ///
    /// Faction updates will only occur once per 24 hour period which resets at 11:05 UTC when the EVE ESI
    /// faction cache expires.
    ///
    /// For simply getting a faction without an update, use [`FactionRepository::get_by_faction_id`]
    pub async fn get_or_update_factions(
        &self,
        faction_id: i64,
    ) -> Result<entity::eve_faction::Model, Error> {
        let faction_repo = FactionRepository::new(self.db);

        let result = faction_repo.get_by_faction_id(faction_id).await?;

        if let Some(faction) = result {
            return Ok(faction);
        };

        // If the faction is not found, then a new patch may have come out adding
        // a new faction. Attempt to update factions if they haven't already been
        // updated since downtime.
        let updated_factions = self.update_factions().await?;

        if let Some(faction) = updated_factions
            .into_iter()
            .find(|f| f.faction_id == faction_id)
        {
            return Ok(faction);
        }

        Err(Error::EveFactionNotFound(faction_id))
    }
}

#[cfg(test)]
mod tests {

    mod update_factions {
        use bifrost_test_utils::prelude::*;
        use chrono::{Duration, Utc};
        use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};

        use crate::server::{
            error::Error, service::eve::faction::FactionService,
            util::time::effective_faction_cache_expiry,
        };

        /// Expect success when updating an empty factions table
        #[tokio::test]
        async fn returns_success_when_updating_empty_faction_table() -> Result<(), TestError> {
            let mut test = test_setup!(entity::prelude::EveFaction)?;
            let faction_endpoint = test.with_faction_endpoint(1, 1);

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let update_result = faction_service.update_factions().await;

            assert!(update_result.is_ok());
            let updated = update_result.unwrap();
            assert!(!updated.is_empty());

            // Assert 1 request was made to mock endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with an update performed due to existing factions being past cache expiry
        #[tokio::test]
        async fn returns_success_with_update_due_to_past_cache_expiry() -> Result<(), TestError> {
            let mut test = test_setup!(entity::prelude::EveFaction)?;
            let faction_model = test.insert_mock_faction(1).await?;
            let faction_endpoint = test.with_faction_endpoint(1, 1);

            // Set updated_at to *before* the effective expiry so an update should be performed.
            let now = Utc::now();
            let effective_expiry = effective_faction_cache_expiry(now).unwrap();
            let updated_at = effective_expiry
                .checked_sub_signed(Duration::minutes(5))
                .unwrap_or(effective_expiry);
            let mut faction_am = faction_model.into_active_model();
            faction_am.updated_at = ActiveValue::Set(updated_at);
            faction_am.update(&test.state.db).await?;

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let result = faction_service.update_factions().await;

            assert!(result.is_ok());
            let updated = result.unwrap();
            assert_eq!(updated.len(), 1);
            let updated_faction = updated.iter().next().unwrap();
            assert!(updated_faction.updated_at > updated_at);

            // Assert 1 request was made to mock endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with no update performed due to existing factions still being within cache period
        #[tokio::test]
        async fn returns_success_with_no_update_due_to_within_cache_expiry() -> Result<(), TestError>
        {
            let mut test = test_setup!(entity::prelude::EveFaction)?;
            let faction_model = test.insert_mock_faction(1).await?;
            let faction_endpoint = test.with_faction_endpoint(1, 0);

            // Set updated_at to just after the effective expiry so it should be considered cached.
            let now = Utc::now();
            let effective_expiry = effective_faction_cache_expiry(now).unwrap();
            let updated_at = effective_expiry
                .checked_add_signed(Duration::minutes(1))
                .unwrap_or(effective_expiry);
            let mut faction_am = faction_model.into_active_model();
            faction_am.updated_at = ActiveValue::Set(updated_at);
            faction_am.update(&test.state.db).await?;

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let result = faction_service.update_factions().await;

            assert!(result.is_ok());
            let updated = result.unwrap();
            assert!(updated.is_empty());

            // Assert no request was made to mock endpoint due to cache
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Error when attempting to update factions while ESI endpoint is unavailable
        #[tokio::test]
        async fn returns_error_due_to_unavailable_esi_endpoint() -> Result<(), TestError> {
            let test = test_setup!(entity::prelude::EveFaction)?;

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let update_result = faction_service.update_factions().await;

            assert!(matches!(
                update_result,
                Err(Error::EsiError(eve_esi::Error::ReqwestError(_)))
            ));

            Ok(())
        }

        /// Expect Error when attempting to update factions due to required tables not being created
        #[tokio::test]
        async fn returns_error_due_to_missing_required_tables() -> Result<(), TestError> {
            let test = test_setup!()?;

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let update_result = faction_service.update_factions().await;

            assert!(matches!(update_result, Err(Error::DbErr(_))));

            Ok(())
        }
    }

    mod get_or_update_factions {
        use bifrost_test_utils::prelude::*;

        use crate::server::{error::Error, service::eve::faction::FactionService};

        /// Expect Ok with faction found when it is present in database
        #[tokio::test]
        async fn returns_success_with_found_when_faction_exists() -> Result<(), TestError> {
            let mut test = test_setup!(entity::prelude::EveFaction)?;
            let faction_model = test.insert_mock_faction(1).await?;
            let faction_endpoint = test.with_faction_endpoint(1, 0);

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let result = faction_service
                .get_or_update_factions(faction_model.faction_id)
                .await;

            assert!(result.is_ok());
            // Assert no requests were made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Ok with created faction when not present in database
        #[tokio::test]
        async fn returns_success_with_created_when_faction_doesnt_exist() -> Result<(), TestError> {
            let mut test = test_setup!(entity::prelude::EveFaction)?;
            let faction_id = 1;
            let faction_endpoint = test.with_faction_endpoint(faction_id, 1);

            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let update_result = faction_service.get_or_update_factions(faction_id).await;

            assert!(update_result.is_ok());
            // Call method one more time to ensure the faction is not retrieved from endpoint again
            let get_result = faction_service.get_or_update_factions(faction_id).await;
            assert!(get_result.is_ok());
            // Assert only 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }

        /// Expect Error when required database tables for factions are missing
        #[tokio::test]
        async fn returns_error_when_required_tables_are_missing() -> Result<(), TestError> {
            let test = test_setup!()?;

            let faction_id = 1;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let result = faction_service.get_or_update_factions(faction_id).await;

            assert!(matches!(result, Err(Error::DbErr(_))));

            Ok(())
        }

        /// Expect Error when ESI endpoint for factions is not available
        #[tokio::test]
        async fn returns_error_when_esi_endpoint_is_unavailable() -> Result<(), TestError> {
            let test = test_setup!(entity::prelude::EveFaction)?;

            let faction_id = 1;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let result = faction_service.get_or_update_factions(faction_id).await;

            assert!(matches!(result, Err(Error::EsiError(_))));

            Ok(())
        }

        /// Expect Error if ESI endpoint does not return the required faction
        #[tokio::test]
        async fn returns_error_when_endpoint_doesnt_have_faction() -> Result<(), TestError> {
            let mut test = test_setup!(entity::prelude::EveFaction)?;
            let faction_endpoint = test.with_faction_endpoint(1, 1);

            let faction_id = 2;
            let faction_service = FactionService::new(&test.state.db, &test.state.esi_client);
            let result = faction_service.get_or_update_factions(faction_id).await;

            assert!(matches!(result, Err(Error::EveFactionNotFound(_))));
            // Assert 1 request was made to faction endpoint
            faction_endpoint.assert();

            Ok(())
        }
    }
}

use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    service::{
        orchestrator::{
            cache::TrackedTransaction, character::CharacterOrchestrator, OrchestrationCache,
        },
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

    /// Upserts information for provided character ID from ESI
    pub async fn upsert(&self, character_id: i64) -> Result<entity::eve_character::Model, Error> {
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

                    let txn = TrackedTransaction::begin(&db).await?;

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
}

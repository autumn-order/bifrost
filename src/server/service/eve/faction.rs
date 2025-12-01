use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    service::{
        orchestrator::{
            cache::TrackedTransaction, faction::FactionOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
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
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry("faction info update", |cache| {
            let db = db.clone();
            let esi_client = esi_client.clone();

            Box::pin(async move {
                let faction_orch = FactionOrchestrator::new(&db, &esi_client);

                let Some(fetched_factions) = faction_orch.fetch_factions(cache).await? else {
                    return Ok(Vec::new());
                };

                let txn = TrackedTransaction::begin(&db).await?;

                let faction_models = faction_orch
                    .persist_factions(&txn, fetched_factions, cache)
                    .await?;

                txn.commit().await?;

                Ok(faction_models)
            })
        })
        .await
    }
}

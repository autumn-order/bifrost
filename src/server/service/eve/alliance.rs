use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    service::{
        orchestrator::{
            alliance::AllianceOrchestrator, cache::TrackedTransaction, OrchestrationCache,
        },
        retry::RetryContext,
    },
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

    /// Upserts information for provided alliance ID from ESI
    pub async fn upsert(&self, alliance_id: i64) -> Result<entity::eve_alliance::Model, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for alliance ID {}", alliance_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let alliance_orch = AllianceOrchestrator::new(&db, &esi_client);

                    let fetched_alliance = alliance_orch.fetch_alliance(alliance_id, cache).await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let model = alliance_orch
                        .persist(&txn, alliance_id, fetched_alliance, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(model)
                })
            },
        )
        .await
    }
}

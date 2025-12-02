use sea_orm::DatabaseConnection;

use crate::server::{
    error::Error,
    service::{
        orchestrator::{
            cache::TrackedTransaction, corporation::CorporationOrchestrator, OrchestrationCache,
        },
        retry::RetryContext,
    },
};

pub struct CorporationService<'a> {
    db: &'a DatabaseConnection,
    esi_client: &'a eve_esi::Client,
}

impl<'a> CorporationService<'a> {
    /// Creates a new instance of [`CorporationService`]
    pub fn new(db: &'a DatabaseConnection, esi_client: &'a eve_esi::Client) -> Self {
        Self { db, esi_client }
    }

    /// Upserts information for provided corporation ID from ESI
    pub async fn upsert(
        &self,
        corporation_id: i64,
    ) -> Result<entity::eve_corporation::Model, Error> {
        let mut ctx: RetryContext<OrchestrationCache> = RetryContext::new();

        let db = self.db.clone();
        let esi_client = self.esi_client.clone();

        ctx.execute_with_retry(
            &format!("info update for corporation ID {}", corporation_id),
            |cache| {
                let db = db.clone();
                let esi_client = esi_client.clone();

                Box::pin(async move {
                    let corporation_orch = CorporationOrchestrator::new(&db, &esi_client);

                    let fetched_corporation = corporation_orch
                        .fetch_corporation(corporation_id, cache)
                        .await?;

                    let txn = TrackedTransaction::begin(&db).await?;

                    let model = corporation_orch
                        .persist(&txn, corporation_id, fetched_corporation, cache)
                        .await?;

                    txn.commit().await?;

                    Ok(model)
                })
            },
        )
        .await
    }
}

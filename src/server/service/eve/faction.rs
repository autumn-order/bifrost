use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::server::{data::eve::faction::FactionRepository, error::Error};

/// Fetches & stores NPC faction information from ESI so long as they aren't within cache period
///
/// The NPC faction cache expires at 11:05 UTC (after downtime)
pub async fn update_factions(
    db: &DatabaseConnection,
    esi_client: &eve_esi::Client,
) -> Result<Vec<entity::eve_faction::Model>, Error> {
    let faction_repo = FactionRepository::new(&db);

    let today = Utc::now().date_naive();
    let expiry_naive = today.and_hms_opt(11, 5, 0);
    let faction_cache_expiry: DateTime<Utc> = match expiry_naive {
        Some(expiry) => DateTime::from_naive_utc_and_offset(expiry, Utc),
        None => return Err(todo!()),
    };

    if let Some(faction) = faction_repo.get_latest().await? {
        // Convert updated_at to Utc time
        let updated_at_utc: DateTime<Utc> =
            DateTime::<Utc>::from_naive_utc_and_offset(faction.updated_at, Utc);

        if updated_at_utc > faction_cache_expiry {
            return Ok(Vec::new());
        }
    }

    // 2. If 11:05 UTC (downtime) has not yet elapsed today, do not update

    // 3. Fetch factions from ESI

    // 4. Upsert into database

    todo!()
}

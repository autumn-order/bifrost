use crate::server::error::Error;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};

/// Returns the "effective" NPC faction cache expiry NaiveDateTime used by the faction service.
///
/// The ESI NPC faction cache expiry is 11:05 UTC. The effective expiry is:
/// - yesterday 11:05 if the current time is before today's 11:05
/// - today at 11:05 otherwise
pub fn effective_faction_cache_expiry(now: DateTime<Utc>) -> Result<NaiveDateTime, Error> {
    let today = now.date_naive();
    let yesterday = today.checked_sub_signed(Duration::days(1)).ok_or_else(|| {
        Error::ParseError(
            "Failed to calculate yesterday's ESI NPC faction cache expiry timestamp".to_string(),
        )
    })?;

    let today_expiry = today
        .and_hms_opt(11, 5, 0)
        .ok_or_else(|| {
            Error::ParseError(
                "Failed to parse hours, minutes, and seconds used to represent ESI NPC faction cache expiry timestamp.".to_string()
            )
        })?;
    let yesterday_expiry = yesterday
        .and_hms_opt(11, 5, 0)
        .ok_or_else(|| {
            Error::ParseError(
                "Failed to parse hours, minutes, and seconds used to represent ESI NPC faction cache expiry timestamp.".to_string()
            )
        })?;

    let now_naive = now.naive_utc();
    Ok(if now_naive < today_expiry {
        yesterday_expiry
    } else {
        today_expiry
    })
}

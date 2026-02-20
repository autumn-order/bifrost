use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use crate::server::error::Error;

/// Calculates the effective ESI NPC faction cache expiry timestamp.
///
/// ESI's NPC faction endpoint has a fixed daily cache expiry at 11:05 UTC. This function
/// determines the most recent expiry timestamp relative to the current time, which is used
/// to decide whether cached faction data needs refresh. This allows the faction service to
/// check if its local cache is stale by comparing the last update timestamp against this
/// effective expiry time.
///
/// The effective expiry is:
/// - Yesterday at 11:05 UTC if the current time is before today's 11:05 UTC
/// - Today at 11:05 UTC if the current time is at or after today's 11:05 UTC
///
/// # Arguments
/// - `now` - Current UTC timestamp to calculate the effective expiry relative to
///
/// # Returns
/// - `Ok(NaiveDateTime)` - The effective faction cache expiry timestamp (either today or yesterday at 11:05 UTC)
/// - `Err(Error::ParseError)` - Failed to calculate yesterday's date or construct the expiry timestamp
pub(super) fn effective_faction_cache_expiry(now: DateTime<Utc>) -> Result<NaiveDateTime, Error> {
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

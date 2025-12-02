//! Configuration constants for scheduler cache durations and cron expressions.
//!
//! This module defines cache durations, scheduling intervals, and cron expressions for all
//! EVE Online entity types that the scheduler manages. Each entity type has its own
//! submodule with constants that control when and how often data is refreshed.

use chrono::Duration;

pub mod eve {
    //! EVE Online entity scheduling configuration.
    //!
    //! Contains configuration submodules for each EVE entity type (factions, alliances,
    //! corporations, characters, and character affiliations), defining their cache durations,
    //! scheduling intervals, and cron expressions.

    use super::*;

    pub mod faction {
        //! Faction information scheduling configuration.
        //!
        //! Factions are NPC entities that change rarely, so they use a long cache duration
        //! with infrequent updates spread across 30-minute intervals.

        use super::*;

        /// Cache ESI faction information for 1 day.
        ///
        /// Factions are static NPC entities that rarely change, so we use a long cache duration
        /// to minimize unnecessary API calls.
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (30 minutes).
        ///
        /// This controls how frequently the scheduler wakes up to check for expired faction data.
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for faction update scheduling.
        ///
        /// Runs every 30 minutes at :17 and :47 past the hour (e.g., 00:17, 00:47, 01:17).
        /// Offset from other entity types to distribute scheduler load.
        pub const CRON_EXPRESSION: &str = "0 17,47 * * * *";
    }

    pub mod alliance {
        //! Alliance information scheduling configuration.
        //!
        //! Alliances are player organizations that can change their name, ticker, or other
        //! metadata. They use a 24-hour cache with 30-minute scheduling intervals.

        use super::*;

        /// Cache ESI alliance information for 1 day.
        ///
        /// Alliance metadata (name, ticker, executor corporation) changes infrequently,
        /// so daily updates are sufficient to keep data reasonably fresh.
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (30 minutes).
        ///
        /// This controls how frequently the scheduler wakes up to check for expired alliance data.
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for alliance update scheduling.
        ///
        /// Runs every 30 minutes at :28 and :58 past the hour (e.g., 00:28, 00:58, 01:28).
        /// Offset from other entity types to distribute scheduler load.
        pub const CRON_EXPRESSION: &str = "0 28,58 * * * *";
    }

    pub mod corporation {
        //! Corporation information scheduling configuration.
        //!
        //! Corporations are player-run organizations that can change their name, ticker,
        //! alliance membership, and other metadata. They use a 24-hour cache with
        //! 30-minute scheduling intervals.

        use super::*;

        /// Cache ESI corporation information for 1 day.
        ///
        /// Corporation metadata (name, ticker, alliance, CEO, etc.) changes occasionally,
        /// so daily updates balance freshness with API efficiency.
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (30 minutes).
        ///
        /// This controls how frequently the scheduler wakes up to check for expired corporation data.
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for corporation update scheduling.
        ///
        /// Runs every 30 minutes at :11 and :41 past the hour (e.g., 00:11, 00:41, 01:11).
        /// Offset from other entity types to distribute scheduler load.
        pub const CRON_EXPRESSION: &str = "0 11,41 * * * *";
    }

    pub mod character {
        //! Character information scheduling configuration.
        //!
        //! Character metadata (name, corporation, birthday, etc.) changes when a character
        //! changes corporations or due to other in-game events. A 30-day cache is used since
        //! character names rarely change once set.

        use super::*;

        /// Cache ESI character information for 30 days.
        ///
        /// Character metadata is relatively stable. Names rarely change, and corporation
        /// changes are tracked separately via affiliation updates. A long cache duration
        /// reduces unnecessary API load.
        pub const CACHE_DURATION: Duration = Duration::days(30);

        /// Interval the schedule cron task is run (30 minutes).
        ///
        /// This controls how frequently the scheduler wakes up to check for expired character data.
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for character update scheduling.
        ///
        /// Runs every 30 minutes at :06 and :36 past the hour (e.g., 00:06, 00:36, 01:06).
        /// Offset from other entity types to distribute scheduler load.
        pub const CRON_EXPRESSION: &str = "0 6,36 * * * *";
    }

    pub mod character_affiliation {
        //! Character affiliation scheduling configuration.
        //!
        //! Character affiliations (corporation, alliance, faction) can change frequently as
        //! players join or leave organizations. A short 1-hour cache with 10-minute scheduling
        //! intervals ensures affiliation data stays current for active players.

        use super::*;

        /// Cache ESI character affiliations for 1 hour.
        ///
        /// Affiliations change whenever a character joins/leaves a corporation or when their
        /// corporation joins/leaves an alliance. The short cache duration ensures we detect
        /// these changes quickly for characters we're actively tracking.
        pub const CACHE_DURATION: Duration = Duration::hours(1);

        /// Interval the schedule cron task is run (10 minutes).
        ///
        /// More frequent than other entities to ensure affiliation changes are detected quickly.
        /// This allows the system to respond to corporation/alliance changes within 10 minutes.
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(10);

        /// Cron expression for character affiliation update scheduling.
        ///
        /// Runs every 10 minutes at :02, :12, :22, :32, :42, :52 past the hour.
        /// More frequent than other updates to keep affiliation data fresh.
        pub const CRON_EXPRESSION: &str = "0 2,12,22,32,42,52 * * * *";
    }
}

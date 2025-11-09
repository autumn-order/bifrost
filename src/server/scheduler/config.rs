use chrono::Duration;

pub mod eve {
    use super::*;

    pub mod faction {
        use super::*;

        /// Cache ESI faction information for 1 day
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (12 hours)
        pub const SCHEDULE_INTERVAL: Duration = Duration::hours(12);

        /// Cron expression for faction update scheduling
        /// Runs every 12 hours at the top of the hour (00:00, 12:00)
        pub const CRON_EXPRESSION: &str = "0 0 */12 * * *";
    }

    pub mod alliance {
        use super::*;

        /// Cache ESI alliance information for 1 day
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (3 hours)
        pub const SCHEDULE_INTERVAL: Duration = Duration::hours(3);

        /// Cron expression for alliance update scheduling
        /// Runs every 3 hours at the top of the hour (00:00, 03:00, 06:00, etc.)
        pub const CRON_EXPRESSION: &str = "0 0 */3 * * *";
    }

    pub mod corporation {
        use super::*;

        /// Cache ESI corporation information for 1 day
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (3 hours)
        pub const SCHEDULE_INTERVAL: Duration = Duration::hours(3);

        /// Cron expression for corporation update scheduling
        /// Runs every 3 hours at the top of the hour (00:00, 03:00, 06:00, etc.)
        pub const CRON_EXPRESSION: &str = "0 0 */3 * * *";
    }

    pub mod character {
        use super::*;

        /// Cache ESI character information for 30 days
        pub const CACHE_DURATION: Duration = Duration::days(30);

        /// Interval the schedule cron task is run (12 hours)
        pub const SCHEDULE_INTERVAL: Duration = Duration::hours(12);

        /// Cron expression for corporation update scheduling
        /// Runs every 12 hours at the top of the hour (00:00, 12:00)
        pub const CRON_EXPRESSION: &str = "0 0 */12 * * *";
    }

    pub mod character_affiliation {
        use super::*;

        /// Cache ESI character affiliations for 1 hour
        pub const CACHE_DURATION: Duration = Duration::hours(1);

        /// Interval the schedule cron task is run (30 minutes)
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for character affiliation update scheduling
        /// Runs every 30 minutes (00:22, 00:52)
        pub const CRON_EXPRESSION: &str = "0 22,52 * * * *";
    }
}

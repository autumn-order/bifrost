use chrono::Duration;

pub mod eve {
    use super::*;

    pub mod faction {
        use super::*;

        /// Cache ESI faction information for 1 day
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (30 minutes)
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for faction update scheduling
        /// Runs every 30 minutes (00:22, 00:52)
        pub const CRON_EXPRESSION: &str = "0 17,47 * * * *";
    }

    pub mod alliance {
        use super::*;

        /// Cache ESI alliance information for 1 day
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (30 minutes)
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for alliance update scheduling
        /// Runs every 30 minutes (00:28, 00:58)
        pub const CRON_EXPRESSION: &str = "0 28,58 * * * *";
    }

    pub mod corporation {
        use super::*;

        /// Cache ESI corporation information for 1 day
        pub const CACHE_DURATION: Duration = Duration::hours(24);

        /// Interval the schedule cron task is run (30 minutes)
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for corporation update scheduling
        /// Runs every 30 minutes (00:11, 00:41)
        pub const CRON_EXPRESSION: &str = "0 11,41 * * * *";
    }

    pub mod character {
        use super::*;

        /// Cache ESI character information for 30 days
        pub const CACHE_DURATION: Duration = Duration::days(30);

        /// Interval the schedule cron task is run (30 minutes)
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(30);

        /// Cron expression for corporation update scheduling
        /// Runs every 30 minutes (00:06, 00:36)
        pub const CRON_EXPRESSION: &str = "0 6,36 * * * *";
    }

    pub mod character_affiliation {
        use super::*;

        /// Cache ESI character affiliations for 1 hour
        pub const CACHE_DURATION: Duration = Duration::hours(1);

        /// Interval the schedule cron task is run (10 minutes)
        /// Ensures affiliations are refreshed within 10 minutes of expiration
        pub const SCHEDULE_INTERVAL: Duration = Duration::minutes(10);

        /// Cron expression: runs every 10 minutes
        /// (at :02, :12, :22, :32, :42, :52 past the hour)
        pub const CRON_EXPRESSION: &str = "0 2,12,22,32,42,52 * * * *";
    }
}

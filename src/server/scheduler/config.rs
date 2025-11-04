use chrono::Duration;

pub mod eve {
    use super::*;

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
}

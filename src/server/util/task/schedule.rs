use chrono::{DateTime, Duration, Utc};

use crate::server::model::worker::WorkerJob;

static MIN_BATCH_LIMIT: i64 = 100;

/// Determines the limit of table entries to schedule an update for based upon schedule interval
pub fn calculate_batch_limit(
    table_entries: u64,
    cache: Duration,
    schedule_interval: Duration,
) -> u64 {
    if table_entries == 0 {
        return 0;
    }

    let batches_per_cache_period = cache.num_minutes() / schedule_interval.num_minutes();

    if batches_per_cache_period > 0 {
        (table_entries / batches_per_cache_period as u64).max(MIN_BATCH_LIMIT as u64)
    } else {
        table_entries
    }
}

/// Staggers provided jobs across the provided update schedule interval
pub async fn create_job_schedule(
    jobs: Vec<(i32, WorkerJob)>,
    schedule_interval: Duration,
) -> Result<Vec<(i32, WorkerJob, DateTime<Utc>)>, crate::server::error::Error> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    let num_jobs = jobs.len() as i64;
    let window_seconds = schedule_interval.num_seconds();
    let base_time = Utc::now();

    let mut scheduled_jobs = Vec::new();

    for (index, (id, job)) in jobs.into_iter().enumerate() {
        // Distribute jobs evenly across the window: (index * window) / total_jobs
        // This allows multiple jobs per second and ensures all jobs fit within the window
        let offset_seconds = (index as i64 * window_seconds) / num_jobs;
        let scheduled_time = base_time + Duration::seconds(offset_seconds);

        scheduled_jobs.push((id, job, scheduled_time))
    }

    Ok(scheduled_jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod calculate_batch_limit {
        use super::*;

        /// Expect 0 when table has no entries
        #[test]
        fn returns_zero_for_empty_table() {
            let result = calculate_batch_limit(0, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 0);
        }

        /// Expect correct batch size for standard case
        #[test]
        fn calculates_standard_batch_size() {
            // 600 entries, 60 min cache, 10 min schedule = 6 batches, 100 per batch
            let result = calculate_batch_limit(600, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect minimum of MIN_BATCH_LIMIT when calculation would result in less
        #[test]
        fn returns_minimum_of_one_hundred() {
            // 5 entries, 60 min cache, 10 min schedule = 6 batches, but min MIN_BATCH_LIMIT per batch
            let result = calculate_batch_limit(5, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect all entries when schedule interval equals cache duration
        #[test]
        fn returns_all_entries_when_interval_equals_cache() {
            // 100 entries, 60 min cache, 60 min schedule = 1 batch, all entries
            let result = calculate_batch_limit(100, Duration::minutes(60), Duration::minutes(60));
            assert_eq!(result, 100);
        }

        /// Expect all entries when schedule interval exceeds cache duration
        #[test]
        fn returns_all_entries_when_interval_exceeds_cache() {
            // 100 entries, 60 min cache, 120 min schedule = 0 batches per period, return all
            let result = calculate_batch_limit(100, Duration::minutes(60), Duration::minutes(120));
            assert_eq!(result, 100);
        }

        /// Expect minimum batch size with single entry
        #[test]
        fn handles_single_entry() {
            let result = calculate_batch_limit(1, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect correct batch size with large number of entries
        #[test]
        fn handles_large_number_of_entries() {
            // 10000 entries, 60 min cache, 10 min schedule = 6 batches, 1666 per batch
            let result = calculate_batch_limit(10000, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 1666);
        }

        /// Expect correct batch size when entries don't divide evenly
        #[test]
        fn handles_uneven_division() {
            // 1000 entries, 60 min cache, 10 min schedule = 6 batches, 166 per batch (1000/6 = 166.66)
            let result = calculate_batch_limit(1000, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 166);
        }

        /// Expect minimum batch size to be applied
        #[test]
        fn applies_minimum_batch_limit() {
            // 50 entries, 60 min cache, 10 min schedule = 6 batches, 8 per batch, but min is MIN_BATCH_LIMIT
            let result = calculate_batch_limit(50, Duration::minutes(60), Duration::minutes(10));
            assert_eq!(result, 100);
        }

        /// Expect correct batch size with different time units
        #[test]
        fn works_with_different_time_units() {
            // 1000 entries, 120 min cache, 30 min schedule = 4 batches, 250 per batch
            let result = calculate_batch_limit(1000, Duration::minutes(120), Duration::minutes(30));
            assert_eq!(result, 250);
        }

        /// Expect correct batch size when cache is slightly longer than schedule
        #[test]
        fn handles_small_cache_to_schedule_ratio() {
            // 100 entries, 15 min cache, 10 min schedule = 1 batch, 100 per batch
            let result = calculate_batch_limit(100, Duration::minutes(15), Duration::minutes(10));
            assert_eq!(result, 100);
        }
    }

    mod create_job_schedule {
        use super::*;

        /// Expect empty vec when no jobs provided
        #[tokio::test]
        async fn returns_empty_for_no_jobs() {
            let result = create_job_schedule(vec![], Duration::minutes(10)).await;

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert!(scheduled_jobs.is_empty());
        }

        /// Expect single job scheduled at current time or shortly after
        #[tokio::test]
        async fn schedules_single_job() {
            let jobs = vec![(1, WorkerJob::UpdateAllianceInfo { alliance_id: 1 })];

            let before = Utc::now().timestamp();
            let result = create_job_schedule(jobs, Duration::minutes(10)).await;
            let after = Utc::now().timestamp();

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert_eq!(scheduled_jobs.len(), 1);

            let (id, job, scheduled_at) = &scheduled_jobs[0];
            assert_eq!(*id, 1);
            assert!(matches!(
                job,
                WorkerJob::UpdateAllianceInfo { alliance_id: 1 }
            ));
            assert!(scheduled_at.timestamp() >= before);
            assert!(scheduled_at.timestamp() <= after + 1); // Allow 1 second for execution time
        }

        /// Expect jobs staggered evenly across schedule interval
        #[tokio::test]
        async fn staggers_job_execution_times() {
            let jobs = vec![
                (1, WorkerJob::UpdateAllianceInfo { alliance_id: 1 }),
                (2, WorkerJob::UpdateAllianceInfo { alliance_id: 2 }),
                (3, WorkerJob::UpdateAllianceInfo { alliance_id: 3 }),
            ];

            let schedule_interval = Duration::minutes(10);
            let before = Utc::now().timestamp();
            let result = create_job_schedule(jobs, schedule_interval).await;

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert_eq!(scheduled_jobs.len(), 3);

            // schedule_interval = 10 minutes = 600 seconds
            // With 3 jobs, interval should be 600 / 3 = 200 seconds
            let expected_interval = 200;

            // Check that scheduled times are properly staggered
            assert!(scheduled_jobs[0].2.timestamp() >= before);
            assert_eq!(
                scheduled_jobs[1].2.timestamp() - scheduled_jobs[0].2.timestamp(),
                expected_interval
            );
            assert_eq!(
                scheduled_jobs[2].2.timestamp() - scheduled_jobs[1].2.timestamp(),
                expected_interval
            );
        }

        /// Expect jobs distributed evenly when more jobs than seconds exist
        #[tokio::test]
        async fn handles_more_jobs_than_seconds() {
            // Create more jobs than seconds in the schedule interval
            let mut jobs = Vec::new();
            for i in 1..=700 {
                jobs.push((i as i32, WorkerJob::UpdateAllianceInfo { alliance_id: i }));
            }

            let schedule_interval = Duration::minutes(10); // 600 seconds
            let before = Utc::now().timestamp();
            let result = create_job_schedule(jobs, schedule_interval).await;
            let after = before + schedule_interval.num_seconds();

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert_eq!(scheduled_jobs.len(), 700);

            // All jobs should fit within the 600-second window
            for (index, (_, _, scheduled_at)) in scheduled_jobs.iter().enumerate() {
                assert!(
                    scheduled_at.timestamp() >= before && scheduled_at.timestamp() <= after,
                    "Job {} scheduled at {} is outside window [{}, {}]",
                    index,
                    scheduled_at.timestamp(),
                    before,
                    after
                );
            }

            // First job should be at or near the start
            assert_eq!(scheduled_jobs[0].2.timestamp(), before);

            // Last job should be near the end but within window
            assert!(scheduled_jobs[699].2.timestamp() <= after);
            assert!(scheduled_jobs[699].2.timestamp() >= after - 2); // Within last 2 seconds of window
        }

        /// Expect correct job structure with WorkerJob and timestamp
        #[tokio::test]
        async fn returns_correct_job_structure() {
            let jobs = vec![
                (1, WorkerJob::UpdateAllianceInfo { alliance_id: 42 }),
                (2, WorkerJob::UpdateAllianceInfo { alliance_id: 99 }),
            ];

            let before = Utc::now().timestamp();
            let result = create_job_schedule(jobs, Duration::minutes(5)).await;
            let after = Utc::now().timestamp() + Duration::minutes(5).num_seconds();

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert_eq!(scheduled_jobs.len(), 2);

            // Verify first job
            let (id1, job1, scheduled_at1) = &scheduled_jobs[0];
            assert_eq!(*id1, 1);
            assert!(matches!(
                job1,
                WorkerJob::UpdateAllianceInfo { alliance_id: 42 }
            ));
            assert!(scheduled_at1.timestamp() >= before);
            assert!(scheduled_at1.timestamp() <= after);

            // Verify second job
            let (id2, job2, scheduled_at2) = &scheduled_jobs[1];
            assert_eq!(*id2, 2);
            assert!(matches!(
                job2,
                WorkerJob::UpdateAllianceInfo { alliance_id: 99 }
            ));
            assert!(scheduled_at2.timestamp() >= before);
            assert!(scheduled_at2.timestamp() <= after);

            // Verify second job is scheduled after first
            assert!(scheduled_at2.timestamp() > scheduled_at1.timestamp());
        }

        /// Expect all jobs scheduled within the schedule interval window
        #[tokio::test]
        async fn schedules_within_interval_window() {
            let jobs = vec![
                (1, WorkerJob::UpdateAllianceInfo { alliance_id: 1 }),
                (2, WorkerJob::UpdateAllianceInfo { alliance_id: 2 }),
                (3, WorkerJob::UpdateAllianceInfo { alliance_id: 3 }),
                (4, WorkerJob::UpdateAllianceInfo { alliance_id: 4 }),
                (5, WorkerJob::UpdateAllianceInfo { alliance_id: 5 }),
            ];

            let schedule_interval = Duration::minutes(10);
            let before = Utc::now().timestamp();
            let result = create_job_schedule(jobs, schedule_interval).await;
            let after = before + schedule_interval.num_seconds();

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();

            // All jobs should be scheduled within the interval window
            for (_, _, scheduled_at) in scheduled_jobs {
                assert!(
                    scheduled_at.timestamp() >= before && scheduled_at.timestamp() <= after,
                    "Job scheduled at {} is outside window [{}, {}]",
                    scheduled_at.timestamp(),
                    before,
                    after
                );
            }
        }

        /// Expect jobs maintain order from input
        #[tokio::test]
        async fn maintains_job_order() {
            let jobs = vec![
                (1, WorkerJob::UpdateAllianceInfo { alliance_id: 10 }),
                (2, WorkerJob::UpdateAllianceInfo { alliance_id: 20 }),
                (3, WorkerJob::UpdateAllianceInfo { alliance_id: 30 }),
                (4, WorkerJob::UpdateAllianceInfo { alliance_id: 40 }),
            ];

            let result = create_job_schedule(jobs, Duration::minutes(10)).await;

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();
            assert_eq!(scheduled_jobs.len(), 4);

            // Jobs should maintain their input order
            assert_eq!(scheduled_jobs[0].0, 1);
            assert!(matches!(
                scheduled_jobs[0].1,
                WorkerJob::UpdateAllianceInfo { alliance_id: 10 }
            ));
            assert_eq!(scheduled_jobs[1].0, 2);
            assert!(matches!(
                scheduled_jobs[1].1,
                WorkerJob::UpdateAllianceInfo { alliance_id: 20 }
            ));
            assert_eq!(scheduled_jobs[2].0, 3);
            assert!(matches!(
                scheduled_jobs[2].1,
                WorkerJob::UpdateAllianceInfo { alliance_id: 30 }
            ));
            assert_eq!(scheduled_jobs[3].0, 4);
            assert!(matches!(
                scheduled_jobs[3].1,
                WorkerJob::UpdateAllianceInfo { alliance_id: 40 }
            ));
        }

        /// Expect timestamps are monotonically increasing
        #[tokio::test]
        async fn produces_monotonic_timestamps() {
            let mut jobs = Vec::new();
            for i in 1..=50 {
                jobs.push((i as i32, WorkerJob::UpdateAllianceInfo { alliance_id: i }));
            }

            let result = create_job_schedule(jobs, Duration::minutes(10)).await;

            assert!(result.is_ok());
            let scheduled_jobs = result.unwrap();

            // Verify timestamps increase monotonically
            for i in 1..scheduled_jobs.len() {
                assert!(
                    scheduled_jobs[i].2.timestamp() >= scheduled_jobs[i - 1].2.timestamp(),
                    "Timestamp at index {} ({}) is not >= previous timestamp ({})",
                    i,
                    scheduled_jobs[i].2.timestamp(),
                    scheduled_jobs[i - 1].2.timestamp()
                );
            }
        }

        /// Expect correct interval calculation for various job counts
        #[tokio::test]
        async fn calculates_correct_intervals_for_different_counts() {
            let test_cases = vec![
                (2, Duration::minutes(10), 300),  // 600 / 2 = 300
                (4, Duration::minutes(8), 120),   // 480 / 4 = 120
                (10, Duration::minutes(5), 30),   // 300 / 10 = 30
                (100, Duration::minutes(20), 12), // 1200 / 100 = 12
            ];

            for (job_count, interval, expected_interval) in test_cases {
                let mut jobs = Vec::new();
                for i in 1..=job_count {
                    jobs.push((i as i32, WorkerJob::UpdateAllianceInfo { alliance_id: i }));
                }

                let result = create_job_schedule(jobs, interval).await;
                assert!(result.is_ok());

                let scheduled_jobs = result.unwrap();
                assert_eq!(scheduled_jobs.len(), job_count as usize);

                // Check intervals between consecutive jobs
                for i in 1..scheduled_jobs.len() {
                    let actual_interval =
                        scheduled_jobs[i].2.timestamp() - scheduled_jobs[i - 1].2.timestamp();
                    assert_eq!(
                        actual_interval, expected_interval,
                        "For {} jobs with {:?} interval, expected interval {} but got {}",
                        job_count, interval, expected_interval, actual_interval
                    );
                }
            }
        }
    }
}

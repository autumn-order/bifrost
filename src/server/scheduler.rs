use sea_orm::DatabaseConnection;
use tokio_cron_scheduler::{JobScheduler, JobSchedulerError};

pub async fn start_job_scheduler(db: &DatabaseConnection) -> Result<(), JobSchedulerError> {
    let sched = JobScheduler::new().await?;

    sched.start().await?;

    Ok(())
}

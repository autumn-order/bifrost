use sea_orm_migration::{
    prelude::{extension::postgres::Type, *},
    schema::*,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

static IDX_UNIQUE_JOB: &str = "idx_bifrost_scheduler_unique_job";
static IDX_PENDING_JOB: &str = "idx_bifrost_scheduler_pending_job";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(JobType::Enum)
                    .values([
                        JobType::FactionInfo,
                        JobType::AllianceInfo,
                        JobType::CorporationInfo,
                        JobType::CharacterInfo,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(JobStatus::Enum)
                    .values([JobStatus::Pending, JobStatus::Processing, JobStatus::Failed])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(BifrostSchedulerQueue::Table)
                    .if_not_exists()
                    .col(pk_auto(BifrostSchedulerQueue::Id))
                    .col(enumeration(
                        BifrostSchedulerQueue::JobType,
                        JobType::Enum,
                        [
                            JobType::FactionInfo,
                            JobType::AllianceInfo,
                            JobType::CorporationInfo,
                            JobType::CharacterInfo,
                        ],
                    ))
                    .col(big_integer(BifrostSchedulerQueue::ResourceId))
                    .col(timestamp(BifrostSchedulerQueue::ScheduledFor))
                    .col(
                        enumeration(
                            BifrostSchedulerQueue::Status,
                            JobStatus::Enum,
                            [JobStatus::Pending, JobStatus::Processing, JobStatus::Failed],
                        )
                        .default("pending"),
                    )
                    .col(
                        timestamp_with_time_zone(BifrostSchedulerQueue::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_UNIQUE_JOB)
                    .table(BifrostSchedulerQueue::Table)
                    .col(BifrostSchedulerQueue::JobType)
                    .col(BifrostSchedulerQueue::ResourceId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_PENDING_JOB)
                    .table(BifrostSchedulerQueue::Table)
                    .col(BifrostSchedulerQueue::Status)
                    .col(BifrostSchedulerQueue::ScheduledFor)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(IDX_PENDING_JOB)
                    .table(BifrostSchedulerQueue::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_UNIQUE_JOB)
                    .table(BifrostSchedulerQueue::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(BifrostSchedulerQueue::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(JobStatus::Enum).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(JobType::Enum).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum JobType {
    #[sea_orm(iden = "bifrost_scheduler_job_type")]
    Enum,
    FactionInfo,
    AllianceInfo,
    CorporationInfo,
    CharacterInfo,
}

#[derive(DeriveIden)]
enum JobStatus {
    #[sea_orm(iden = "bifrost_scheduler_job_status")]
    Enum,
    Pending,
    Processing,
    Failed,
}

#[derive(DeriveIden)]
enum BifrostSchedulerQueue {
    Table,
    Id,
    JobType,
    ResourceId,
    ScheduledFor,
    Status,
    CreatedAt,
}

use sea_orm_migration::{prelude::*, schema::*};

use crate::m20251017_000001_create_eve_faction_table::EveFaction;

static IDX_EVE_ALLIANCE_FACTION_ID: &str = "idx_eve_alliance_faction_id";
static IDX_EVE_ALLIANCE_UPDATED_AT: &str = "idx_eve_alliance_updated_at";
static FK_EVE_ALLIANCE_FACTION_ID: &str = "fk_eve_alliance_faction_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EveAlliance::Table)
                    .if_not_exists()
                    .col(pk_auto(EveAlliance::Id))
                    .col(big_integer_uniq(EveAlliance::AllianceId))
                    .col(integer_null(EveAlliance::FactionId))
                    .col(big_integer(EveAlliance::CreatorCorporationId))
                    .col(big_integer_null(EveAlliance::ExecutorCorporationId))
                    .col(big_integer(EveAlliance::CreatorId))
                    .col(timestamp(EveAlliance::DateFounded))
                    .col(string(EveAlliance::Name))
                    .col(string(EveAlliance::Ticker))
                    .col(timestamp(EveAlliance::CreatedAt))
                    .col(timestamp(EveAlliance::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_ALLIANCE_FACTION_ID)
                    .table(EveAlliance::Table)
                    .col(EveAlliance::FactionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_ALLIANCE_UPDATED_AT)
                    .table(EveAlliance::Table)
                    .col(EveAlliance::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_ALLIANCE_FACTION_ID)
                    .from_tbl(EveAlliance::Table)
                    .from_col(EveAlliance::FactionId)
                    .to_tbl(EveFaction::Table)
                    .to_col(EveFaction::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_ALLIANCE_FACTION_ID)
                    .table(EveAlliance::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_ALLIANCE_UPDATED_AT)
                    .table(EveAlliance::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_ALLIANCE_FACTION_ID)
                    .table(EveAlliance::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(EveAlliance::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum EveAlliance {
    Table,
    Id,
    AllianceId,
    FactionId,
    CreatorCorporationId,
    ExecutorCorporationId,
    CreatorId,
    DateFounded,
    Name,
    Ticker,
    CreatedAt,
    UpdatedAt,
}

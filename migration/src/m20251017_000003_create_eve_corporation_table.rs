use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000001_create_eve_faction_table::EveFaction,
    m20251017_000002_create_eve_alliance_table::EveAlliance,
};

static IDX_EVE_CORPORATION_ALLIANCE_ID: &str = "idx_eve_corporation_alliance_id";
static IDX_EVE_CORPORATION_FACTION_ID: &str = "idx_eve_corporation_faction_id";
static FK_EVE_CORPORATION_ALLIANCE_ID: &str = "fk_eve_corporation_alliance_id";
static FK_EVE_CORPORATION_FACTION_ID: &str = "fk_eve_corporation_faction_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EveCorporation::Table)
                    .if_not_exists()
                    .col(pk_auto(EveCorporation::Id))
                    .col(big_integer_uniq(EveCorporation::CorporationId))
                    .col(integer_null(EveCorporation::AllianceId))
                    .col(integer_null(EveCorporation::FactionId))
                    .col(big_integer(EveCorporation::CeoId))
                    .col(big_integer(EveCorporation::CreatorId))
                    .col(timestamp_null(EveCorporation::DateFounded))
                    .col(text_null(EveCorporation::Description))
                    .col(big_integer_null(EveCorporation::HomeStationId))
                    .col(big_integer(EveCorporation::MemberCount))
                    .col(string(EveCorporation::Name))
                    .col(big_integer_null(EveCorporation::Shares))
                    .col(double(EveCorporation::TaxRate))
                    .col(string(EveCorporation::Ticker))
                    .col(string_null(EveCorporation::Url))
                    .col(boolean_null(EveCorporation::WarEligible))
                    .col(timestamp(EveCorporation::CreatedAt))
                    .col(timestamp(EveCorporation::UpdatedAt))
                    .col(timestamp_null(EveCorporation::JobScheduledAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_CORPORATION_ALLIANCE_ID)
                    .table(EveCorporation::Table)
                    .col(EveCorporation::AllianceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_CORPORATION_FACTION_ID)
                    .table(EveCorporation::Table)
                    .col(EveCorporation::FactionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_CORPORATION_ALLIANCE_ID)
                    .from_tbl(EveCorporation::Table)
                    .from_col(EveCorporation::AllianceId)
                    .to_tbl(EveAlliance::Table)
                    .to_col(EveAlliance::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_CORPORATION_FACTION_ID)
                    .from_tbl(EveCorporation::Table)
                    .from_col(EveCorporation::FactionId)
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
                    .name(FK_EVE_CORPORATION_FACTION_ID)
                    .table(EveCorporation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_CORPORATION_ALLIANCE_ID)
                    .table(EveCorporation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_CORPORATION_FACTION_ID)
                    .table(EveCorporation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_CORPORATION_ALLIANCE_ID)
                    .table(EveCorporation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(EveCorporation::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum EveCorporation {
    Table,
    Id,
    CorporationId,
    AllianceId,
    FactionId,
    CeoId,
    CreatorId,
    DateFounded,
    Description,
    HomeStationId,
    MemberCount,
    Name,
    Shares,
    TaxRate,
    Ticker,
    Url,
    WarEligible,
    CreatedAt,
    UpdatedAt,
    JobScheduledAt,
}

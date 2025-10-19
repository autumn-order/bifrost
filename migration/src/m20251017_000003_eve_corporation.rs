use sea_orm_migration::{prelude::*, schema::*};

use crate::{m20251017_000001_eve_faction::EveFaction, m20251017_000002_eve_alliance::EveAlliance};

static IDX_EVE_CORPORATION_ALLIANCE_ID: &str = "idx-eve_corporation-alliance_id";
static IDX_EVE_CORPORATION_FACTION_ID: &str = "idx-eve_corporation-faction_id";
static FK_EVE_CORPORATION_ALLIANCE_ID: &str = "fk-eve_corporation-alliance_id";
static FK_EVE_CORPORATION_FACTION_ID: &str = "fk-eve_corporation-faction_id";
static FK_EVE_FACTION_CORPORATION_ID: &str = "fk-eve_faction-corporation_id";
static FK_EVE_FACTION_MILITIA_CORPORATION_ID: &str = "fk-eve_faction-militia_corporation_id";
static FK_EVE_ALLIANCE_CREATOR_CORPORATION_ID: &str = "fk-eve_alliance-creator_corporation_id";
static FK_EVE_ALLIANCE_EXECUTOR_CORPORATION_ID: &str = "fk-eve_alliance-executor_corporation_id";

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
                    .col(integer(EveCorporation::CeoId))
                    .col(integer(EveCorporation::CreatorId))
                    .col(timestamp_null(EveCorporation::DateFounded))
                    .col(text_null(EveCorporation::Description))
                    .col(big_integer(EveCorporation::HomeStationId))
                    .col(integer(EveCorporation::MemberCount))
                    .col(string(EveCorporation::Name))
                    .col(big_integer_null(EveCorporation::Shares))
                    .col(integer(EveCorporation::TaxRate))
                    .col(string(EveCorporation::Ticker))
                    .col(string_null(EveCorporation::Url))
                    .col(boolean(EveCorporation::WarEligible))
                    .col(timestamp(EveCorporation::CreatedAt))
                    .col(timestamp(EveCorporation::UpdatedAt))
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

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_FACTION_CORPORATION_ID)
                    .from_tbl(EveFaction::Table)
                    .from_col(EveFaction::CorporationId)
                    .to_tbl(EveCorporation::Table)
                    .to_col(EveCorporation::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_FACTION_MILITIA_CORPORATION_ID)
                    .from_tbl(EveFaction::Table)
                    .from_col(EveFaction::MilitiaCorporationId)
                    .to_tbl(EveCorporation::Table)
                    .to_col(EveCorporation::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_ALLIANCE_CREATOR_CORPORATION_ID)
                    .from_tbl(EveAlliance::Table)
                    .from_col(EveAlliance::CreatorCorporationId)
                    .to_tbl(EveCorporation::Table)
                    .to_col(EveCorporation::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_ALLIANCE_EXECUTOR_CORPORATION_ID)
                    .from_tbl(EveAlliance::Table)
                    .from_col(EveAlliance::ExecutorCorporationId)
                    .to_tbl(EveCorporation::Table)
                    .to_col(EveCorporation::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_ALLIANCE_EXECUTOR_CORPORATION_ID)
                    .table(EveAlliance::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_ALLIANCE_CREATOR_CORPORATION_ID)
                    .table(EveAlliance::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_FACTION_MILITIA_CORPORATION_ID)
                    .table(EveFaction::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_FACTION_CORPORATION_ID)
                    .table(EveFaction::Table)
                    .to_owned(),
            )
            .await?;

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
}

use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000001_eve_faction::EveFaction, m20251017_000002_eve_alliance::EveAlliance,
    m20251017_000003_eve_corporation::EveCorporation,
};

static IDX_EVE_CHARACTER_CORPORATION_ID: &str = "idx-eve_character-corporation_id";
static IDX_EVE_CHARACTER_FACTION_ID: &str = "idx-eve_character-faction_id";
static FK_EVE_CHARACTER_CORPORATION_ID: &str = "fk-eve_character-corporation_id";
static FK_EVE_CHARACTER_FACTION_ID: &str = "fk-eve_character-faction_id";
static FK_EVE_ALLIANCE_CREATOR_ID: &str = "fk-eve_alliance-creator_id";
static FK_EVE_CORPORATION_CREATOR_ID: &str = "fk-eve_corporation-creator_id";
static FK_EVE_CORPORATION_CEO_ID: &str = "fk-eve_corporation-ceo_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EveCharacter::Table)
                    .if_not_exists()
                    .col(pk_auto(EveCharacter::Id))
                    .col(big_integer_uniq(EveCharacter::CharacterId))
                    .col(big_integer(EveCharacter::CorporationId))
                    .col(big_integer_null(EveCharacter::FactionId))
                    .col(date_time(EveCharacter::Birthday))
                    .col(big_integer(EveCharacter::BloodlineId))
                    .col(text_null(EveCharacter::Description))
                    .col(string(EveCharacter::Gender))
                    .col(string(EveCharacter::Name))
                    .col(big_integer(EveCharacter::RaceId))
                    .col(double_null(EveCharacter::SecurityStatus))
                    .col(string_null(EveCharacter::Title))
                    .col(timestamp(EveCharacter::CreatedAt))
                    .col(timestamp(EveCharacter::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_CHARACTER_CORPORATION_ID)
                    .table(EveCharacter::Table)
                    .col(EveCharacter::CorporationId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_CHARACTER_FACTION_ID)
                    .table(EveCharacter::Table)
                    .col(EveCharacter::FactionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_CHARACTER_CORPORATION_ID)
                    .from_tbl(EveCharacter::Table)
                    .from_col(EveCharacter::CorporationId)
                    .to_tbl(EveCorporation::Table)
                    .to_col(EveCorporation::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_CHARACTER_FACTION_ID)
                    .from_tbl(EveCharacter::Table)
                    .from_col(EveCharacter::FactionId)
                    .to_tbl(EveFaction::Table)
                    .to_col(EveFaction::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_ALLIANCE_CREATOR_ID)
                    .from_tbl(EveAlliance::Table)
                    .from_col(EveAlliance::CreatorId)
                    .to_tbl(EveCharacter::Table)
                    .to_col(EveCharacter::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_CORPORATION_CREATOR_ID)
                    .from_tbl(EveCorporation::Table)
                    .from_col(EveCorporation::CreatorId)
                    .to_tbl(EveCharacter::Table)
                    .to_col(EveCharacter::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_EVE_CORPORATION_CEO_ID)
                    .from_tbl(EveCorporation::Table)
                    .from_col(EveCorporation::CeoId)
                    .to_tbl(EveCharacter::Table)
                    .to_col(EveCharacter::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_CORPORATION_CEO_ID)
                    .table(EveCorporation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_CORPORATION_CREATOR_ID)
                    .table(EveCorporation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_ALLIANCE_CREATOR_ID)
                    .table(EveAlliance::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_CHARACTER_FACTION_ID)
                    .table(EveCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_EVE_CHARACTER_CORPORATION_ID)
                    .table(EveCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_CHARACTER_FACTION_ID)
                    .table(EveCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_CHARACTER_CORPORATION_ID)
                    .table(EveCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(EveCharacter::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum EveCharacter {
    Table,
    Id,
    CharacterId,
    CorporationId,
    FactionId,
    Birthday,
    BloodlineId,
    Description,
    Gender,
    Name,
    RaceId,
    SecurityStatus,
    Title,
    CreatedAt,
    UpdatedAt,
}

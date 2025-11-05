use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000001_create_eve_faction_table::EveFaction,
    m20251017_000003_create_eve_corporation_table::EveCorporation,
};

static IDX_EVE_CHARACTER_CORPORATION_ID: &str = "idx_eve_character_corporation_id";
static IDX_EVE_CHARACTER_FACTION_ID: &str = "idx_eve_character_faction_id";
static FK_EVE_CHARACTER_CORPORATION_ID: &str = "fk_eve_character_corporation_id";
static FK_EVE_CHARACTER_FACTION_ID: &str = "fk_eve_character_faction_id";

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
                    .col(integer(EveCharacter::CorporationId))
                    .col(integer_null(EveCharacter::FactionId))
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
                    .col(timestamp_null(EveCharacter::JobScheduledAt))
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

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
    JobScheduledAt,
}

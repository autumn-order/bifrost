use sea_orm_migration::{prelude::*, schema::*};

static IDX_EVE_FACTION_UPDATED_AT: &str = "idx_eve_faction_updated_at";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EveFaction::Table)
                    .if_not_exists()
                    .col(pk_auto(EveFaction::Id))
                    .col(big_integer_uniq(EveFaction::FactionId))
                    .col(big_integer_null(EveFaction::CorporationId))
                    .col(big_integer_null(EveFaction::MilitiaCorporationId))
                    .col(text(EveFaction::Description))
                    .col(boolean(EveFaction::IsUnique))
                    .col(string(EveFaction::Name))
                    .col(double(EveFaction::SizeFactor))
                    .col(big_integer_null(EveFaction::SolarSystemId))
                    .col(big_integer(EveFaction::StationCount))
                    .col(big_integer(EveFaction::StationSystemCount))
                    .col(timestamp(EveFaction::CreatedAt))
                    .col(timestamp(EveFaction::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_EVE_FACTION_UPDATED_AT)
                    .table(EveFaction::Table)
                    .col(EveFaction::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(IDX_EVE_FACTION_UPDATED_AT)
                    .table(EveFaction::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(EveFaction::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum EveFaction {
    Table,
    Id,
    FactionId,
    CorporationId,
    MilitiaCorporationId,
    Description,
    IsUnique,
    Name,
    SizeFactor,
    SolarSystemId,
    StationCount,
    StationSystemCount,
    CreatedAt,
    UpdatedAt,
}

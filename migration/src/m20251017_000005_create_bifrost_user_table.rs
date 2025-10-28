use sea_orm_migration::{prelude::*, schema::*};

use crate::m20251017_000004_create_eve_character_table::EveCharacter;

static FK_USER_MAIN_CHARACTER_ID: &str = "fk_bifrost_user_main_character_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BifrostUser::Table)
                    .if_not_exists()
                    .col(pk_auto(BifrostUser::Id))
                    .col(integer(BifrostUser::MainCharacterId))
                    .col(timestamp(BifrostUser::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_MAIN_CHARACTER_ID)
                    .from_tbl(BifrostUser::Table)
                    .from_col(BifrostUser::MainCharacterId)
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
                    .name(FK_USER_MAIN_CHARACTER_ID)
                    .table(BifrostUser::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(BifrostUser::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum BifrostUser {
    Table,
    Id,
    MainCharacterId,
    CreatedAt,
}

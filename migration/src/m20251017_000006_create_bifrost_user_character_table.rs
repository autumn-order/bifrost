use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000004_create_eve_character_table::EveCharacter,
    m20251017_000005_create_bifrost_user_table::BifrostUser,
};

static IDX_USER_CHARACTER_USER_ID: &str = "idx_bifrost_user_character_user_id";
static FK_USER_CHARACTER_USER_ID: &str = "fk_bifrost_user_character_user_id";
static FK_USER_CHARACTER_CHARACTER_ID: &str = "fk_bifrost_user_character_character_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BifrostUserCharacter::Table)
                    .if_not_exists()
                    .col(pk_auto(BifrostUserCharacter::Id))
                    .col(integer(BifrostUserCharacter::UserId))
                    .col(integer_uniq(BifrostUserCharacter::CharacterId))
                    .col(string(BifrostUserCharacter::OwnerHash))
                    .col(timestamp(BifrostUserCharacter::CreatedAt))
                    .col(timestamp(BifrostUserCharacter::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_USER_CHARACTER_USER_ID)
                    .table(BifrostUserCharacter::Table)
                    .col(BifrostUserCharacter::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_USER_ID)
                    .from_tbl(BifrostUserCharacter::Table)
                    .from_col(BifrostUserCharacter::UserId)
                    .to_tbl(BifrostUser::Table)
                    .to_col(BifrostUser::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_CHARACTER_ID)
                    .from_tbl(BifrostUserCharacter::Table)
                    .from_col(BifrostUserCharacter::CharacterId)
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
                    .name(FK_USER_CHARACTER_CHARACTER_ID)
                    .table(BifrostUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_USER_ID)
                    .table(BifrostUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_USER_CHARACTER_USER_ID)
                    .table(BifrostUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(BifrostUserCharacter::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BifrostUserCharacter {
    Table,
    Id,
    UserId,
    CharacterId,
    OwnerHash,
    CreatedAt,
    UpdatedAt,
}

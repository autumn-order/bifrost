use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000004_create_eve_character_table::EveCharacter,
    m20251017_000005_create_bifrost_auth_user_table::BifrostAuthUser,
};

static IDX_USER_CHARACTER_USER_ID: &str = "idx_bifrost_auth_user_character_user_id";
static FK_USER_CHARACTER_USER_ID: &str = "fk_bifrost_auth_user_character_user_id";
static FK_USER_CHARACTER_CHARACTER_ID: &str = "fk_bifrost_auth_user_character_character_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BifrostAuthUserCharacter::Table)
                    .if_not_exists()
                    .col(pk_auto(BifrostAuthUserCharacter::Id))
                    .col(integer(BifrostAuthUserCharacter::UserId))
                    .col(integer_uniq(BifrostAuthUserCharacter::CharacterId))
                    .col(timestamp(BifrostAuthUserCharacter::CreatedAt))
                    .col(timestamp(BifrostAuthUserCharacter::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_USER_CHARACTER_USER_ID)
                    .table(BifrostAuthUserCharacter::Table)
                    .col(BifrostAuthUserCharacter::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_USER_ID)
                    .from_tbl(BifrostAuthUserCharacter::Table)
                    .from_col(BifrostAuthUserCharacter::UserId)
                    .to_tbl(BifrostAuthUser::Table)
                    .to_col(BifrostAuthUser::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_CHARACTER_ID)
                    .from_tbl(BifrostAuthUserCharacter::Table)
                    .from_col(BifrostAuthUserCharacter::CharacterId)
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
                    .table(BifrostAuthUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_USER_ID)
                    .table(BifrostAuthUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_USER_CHARACTER_USER_ID)
                    .table(BifrostAuthUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(BifrostAuthUserCharacter::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BifrostAuthUserCharacter {
    Table,
    Id,
    UserId,
    CharacterId,
    CreatedAt,
    UpdatedAt,
}

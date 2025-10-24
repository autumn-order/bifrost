use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000004_create_eve_character_table::EveCharacter,
    m20251017_000005_create_bifrost_user_table::BifrostUser,
};

static IDX_USER_CHARACTER_HISTORY_CHARACTER_ID: &str =
    "idx_bifrost_user_character_history_character_id";
static FK_USER_CHARACTER_HISTORY_CHARACTER_ID: &str =
    "fk_bifrost_user_character_history_character_id";
static FK_USER_CHARACTER_HISTORY_NEW_USER_ID: &str =
    "fk_bifrost_user_character_history_new_user_id";
static FK_USER_CHARACTER_HISTORY_OLD_USER_ID: &str =
    "fk_bifrost_user_character_history_old_user_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BifrostUserCharacterHistory::Table)
                    .if_not_exists()
                    .col(pk_auto(BifrostUserCharacterHistory::Id))
                    .col(integer(BifrostUserCharacterHistory::CharacterId))
                    .col(integer(BifrostUserCharacterHistory::NewUserId))
                    .col(integer(BifrostUserCharacterHistory::PreviousUserId))
                    .col(timestamp(BifrostUserCharacterHistory::DateTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .table(BifrostUserCharacterHistory::Table)
                    .col(BifrostUserCharacterHistory::CharacterId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .from_tbl(BifrostUserCharacterHistory::Table)
                    .from_col(BifrostUserCharacterHistory::CharacterId)
                    .to_tbl(EveCharacter::Table)
                    .to_col(EveCharacter::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_HISTORY_NEW_USER_ID)
                    .from_tbl(BifrostUserCharacterHistory::Table)
                    .from_col(BifrostUserCharacterHistory::NewUserId)
                    .to_tbl(BifrostUser::Table)
                    .to_col(BifrostUser::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_HISTORY_OLD_USER_ID)
                    .from_tbl(BifrostUserCharacterHistory::Table)
                    .from_col(BifrostUserCharacterHistory::PreviousUserId)
                    .to_tbl(BifrostUser::Table)
                    .to_col(BifrostUser::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_HISTORY_OLD_USER_ID)
                    .table(BifrostUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_HISTORY_NEW_USER_ID)
                    .table(BifrostUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .table(BifrostUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(IDX_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .table(EveCharacter::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(BifrostUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BifrostUserCharacterHistory {
    Table,
    Id,
    CharacterId,
    NewUserId,
    PreviousUserId,
    DateTime,
}

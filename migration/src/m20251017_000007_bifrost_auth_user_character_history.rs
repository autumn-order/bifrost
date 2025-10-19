use sea_orm_migration::{prelude::*, schema::*};

use crate::{
    m20251017_000004_eve_character::EveCharacter,
    m20251017_000005_bifrost_auth_user::BifrostAuthUser,
};

static IDX_USER_CHARACTER_HISTORY_CHARACTER_ID: &str =
    "idx-bifrost_auth_user_character_history-character_id";
static FK_USER_CHARACTER_HISTORY_CHARACTER_ID: &str =
    "fk-bifrost_auth_user_character_history-character_id";
static FK_USER_CHARACTER_HISTORY_NEW_USER_ID: &str =
    "fk-bifrost_auth_user_character_history-new_user_id";
static FK_USER_CHARACTER_HISTORY_OLD_USER_ID: &str =
    "fk-bifrost_auth_user_character_history-old_user_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BifrostAuthUserCharacterHistory::Table)
                    .if_not_exists()
                    .col(pk_auto(BifrostAuthUserCharacterHistory::Id))
                    .col(integer(BifrostAuthUserCharacterHistory::CharacterId))
                    .col(integer(BifrostAuthUserCharacterHistory::NewUserId))
                    .col(integer(BifrostAuthUserCharacterHistory::PreviousUserId))
                    .col(timestamp(BifrostAuthUserCharacterHistory::DateTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(IDX_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .table(BifrostAuthUserCharacterHistory::Table)
                    .col(BifrostAuthUserCharacterHistory::CharacterId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .from_tbl(BifrostAuthUserCharacterHistory::Table)
                    .from_col(BifrostAuthUserCharacterHistory::CharacterId)
                    .to_tbl(EveCharacter::Table)
                    .to_col(EveCharacter::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_HISTORY_NEW_USER_ID)
                    .from_tbl(BifrostAuthUserCharacterHistory::Table)
                    .from_col(BifrostAuthUserCharacterHistory::NewUserId)
                    .to_tbl(BifrostAuthUser::Table)
                    .to_col(BifrostAuthUser::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name(FK_USER_CHARACTER_HISTORY_OLD_USER_ID)
                    .from_tbl(BifrostAuthUserCharacterHistory::Table)
                    .from_col(BifrostAuthUserCharacterHistory::PreviousUserId)
                    .to_tbl(BifrostAuthUser::Table)
                    .to_col(BifrostAuthUser::Id)
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
                    .table(BifrostAuthUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_HISTORY_NEW_USER_ID)
                    .table(BifrostAuthUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name(FK_USER_CHARACTER_HISTORY_CHARACTER_ID)
                    .table(BifrostAuthUserCharacterHistory::Table)
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
                    .table(BifrostAuthUserCharacterHistory::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BifrostAuthUserCharacterHistory {
    Table,
    Id,
    CharacterId,
    NewUserId,
    PreviousUserId,
    DateTime,
}

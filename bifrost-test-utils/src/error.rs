use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestError {
    #[error(transparent)]
    EsiError(#[from] eve_esi::Error),
    #[error(transparent)]
    DbErr(#[from] sea_orm::DbErr),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
}

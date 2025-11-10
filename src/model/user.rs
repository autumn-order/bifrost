use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct UserDto {
    pub id: i32,
    pub character_id: i64,
    pub character_name: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct CharacterDto {
    pub id: i64,
    pub name: String,
    pub corporation: CorporationDto,
    pub alliance: Option<AllianceDto>,
    pub info_updated_at: NaiveDateTime,
    pub affiliation_updated_at: NaiveDateTime,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct CorporationDto {
    pub id: i64,
    pub name: String,
    pub info_updated_at: NaiveDateTime,
    pub affiliation_updated_at: NaiveDateTime,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct AllianceDto {
    pub id: i64,
    pub name: String,
    pub updated_at: NaiveDateTime,
}

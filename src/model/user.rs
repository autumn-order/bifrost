use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct UserDto {
    pub id: i32,
    pub character_id: i64,
    pub character_name: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct CharacterDto {
    pub id: i64,
    pub name: String,
}

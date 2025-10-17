use serde::Serialize;

#[derive(Serialize)]
pub struct Character {
    pub character_id: i64,
    pub character_name: String,
}

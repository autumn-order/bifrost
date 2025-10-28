use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct UserDto {
    pub id: i32,
    pub main_character: Character,
    pub characters: Vec<Character>,
}

#[derive(Serialize, Deserialize)]
pub struct Character {
    pub id: i64,
    pub name: String,
}

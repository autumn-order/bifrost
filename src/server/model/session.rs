use serde::{Deserialize, Serialize};

pub const AUTH_LOGIN_CSRF_KEY: &str = "auth:login:csrf";

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct AuthLoginCsrf(pub String);

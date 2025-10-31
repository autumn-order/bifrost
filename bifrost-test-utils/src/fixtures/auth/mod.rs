pub mod mock;
pub mod mockito;

use crate::TestSetup;

impl TestSetup {
    pub fn auth<'a>(&'a mut self) -> AuthFixtures<'a> {
        AuthFixtures { setup: self }
    }
}

pub struct AuthFixtures<'a> {
    setup: &'a mut TestSetup,
}

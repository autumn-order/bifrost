use crate::TestSetup;

pub mod data;
pub mod mock;
pub mod mockito;

impl TestSetup {
    pub fn eve<'a>(&'a mut self) -> EveFixtures<'a> {
        EveFixtures { setup: self }
    }
}

pub struct EveFixtures<'a> {
    pub setup: &'a mut TestSetup,
}

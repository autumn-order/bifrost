use crate::TestContext;

pub mod data;
pub mod factory;
pub mod mock;
pub mod mockito;

impl TestContext {
    pub fn eve<'a>(&'a mut self) -> EveFixtures<'a> {
        EveFixtures { setup: self }
    }
}

pub struct EveFixtures<'a> {
    pub setup: &'a mut TestContext,
}

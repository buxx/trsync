use anyhow::Result;
use trsync_core::client::TracimClient;

use crate::state::{modification::StateModification, State};

pub mod disk;
pub mod remote;

pub trait Executor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification>;
}

use trsync_core::{client::TracimClient, error::ExecutorError};

use crate::{
    event::Event,
    state::{modification::StateModification, State},
};

pub mod disk;
pub mod remote;

pub trait Executor {
    fn execute(
        &self,
        state: &dyn State,
        tracim: &dyn TracimClient,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError>;
}

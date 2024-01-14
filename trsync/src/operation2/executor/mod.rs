use thiserror::Error;
use trsync_core::{
    client::{TracimClient, TracimClientError},
    instance::ContentId,
};

use crate::{
    event::Event,
    state::{modification::StateModification, State, StateError},
};

pub mod disk;
pub mod remote;

pub trait Executor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError>;
}

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Unexpected error: {0:#}")]
    Unexpected(#[from] anyhow::Error),
    #[error("Tracim error: {0}")]
    Tracim(#[from] TracimClientError),
    #[error("State manipulation error: {0}")]
    State(#[from] StateError),
    #[error("Missing parent {1} for content {0}")]
    MissingParent(ContentId, ContentId),
    #[error("Programmatic error : {0}")]
    Programmatic(String),
    #[error(
        "After receive an Tracim ContentAlreadyExist error, unable to found the content ({0})"
    )]
    NotFoundAfterContentAlreadyExist(String),
    #[error("Maximum retry reached for : {0} (because time out)")]
    MaximumRetryCount(String),
}

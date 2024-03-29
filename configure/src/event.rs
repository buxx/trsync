use trsync_core::instance::{InstanceId, Workspace};

use crate::panel::instance::GuiInstance;

#[derive(Debug)]
pub enum Event {
    GlobalConfigurationUpdated,
    InstanceCredentialsUpdated(GuiInstance),
    ValidateNewInstance(GuiInstance),
    InstanceCredentialsAccepted(GuiInstance),
    InstanceCredentialsRefused(GuiInstance),
    InstanceCredentialsFailed(GuiInstance, String),
    InstanceWorkspacesRetrievedSuccess(InstanceId, Vec<Workspace>),
    InstanceWorkspacesRetrievedFailure(InstanceId, String),
    InstanceSelectedWorkspacesValidated(GuiInstance),
    DeleteInstanceWanted(InstanceId),
}

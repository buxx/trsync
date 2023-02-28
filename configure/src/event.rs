use trsync_core::instance::{Instance, InstanceId, Workspace};

#[derive(Debug)]
pub enum Event {
    GlobalConfigurationUpdated,
    InstanceCredentialsUpdated(Instance),
    InstanceCredentialsAccepted(Instance),
    InstanceCredentialsRefused(Instance),
    InstanceCredentialsFailed(Instance, String),
    InstanceWorkspacesRetrievedSuccess(InstanceId, Vec<Workspace>),
    InstanceWorkspacesRetrievedFailure(InstanceId, String),
}

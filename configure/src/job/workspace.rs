use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use trsync_core::{client::Client, instance::Workspace};

use crate::{event::Event, panel::instance::GuiInstance};

pub struct WorkspacesGrabber {
    event_sender: Sender<Event>,
    instance: GuiInstance,
}

impl WorkspacesGrabber {
    pub fn new(event_sender: Sender<Event>, instance: GuiInstance) -> Self {
        Self {
            event_sender,
            instance,
        }
    }

    pub fn execute(&self) {
        if let Err(error) = match self.get_workspaces() {
            Ok(workspaces) => self
                .event_sender
                .send(Event::InstanceWorkspacesRetrievedSuccess(
                    self.instance.name.clone(),
                    workspaces,
                )),
            Err(error) => self
                .event_sender
                .send(Event::InstanceWorkspacesRetrievedFailure(
                    self.instance.name.clone(),
                    format!("{}", error),
                )),
        } {
            eprintln!(
                "Channel communication error during workspaces grabber : {}",
                error
            )
        }
    }

    fn get_workspaces(&self) -> Result<Vec<Workspace>> {
        Client::new(
            self.instance.api_url(None),
            self.instance.username.clone(),
            self.instance.password.clone(),
        )
        .context("Construct http client")?
        .workspaces()
        .context("Grab workspaces")
    }
}

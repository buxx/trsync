use anyhow::{Context, Result};
use crossbeam_channel::Sender;

use trsync_core::{client::Client, instance::Instance};

use crate::event::Event;

pub struct CredentialUpdater {
    event_sender: Sender<Event>,
    instance: Instance,
}

impl CredentialUpdater {
    pub fn new(event_sender: Sender<Event>, instance: Instance) -> Self {
        Self {
            event_sender,
            instance,
        }
    }

    pub fn execute(&self) {
        if let Err(error) = match self.check_credentials() {
            Ok(valid) => {
                if valid {
                    self.event_sender
                        .send(Event::InstanceCredentialsAccepted(self.instance.clone()))
                } else {
                    self.event_sender
                        .send(Event::InstanceCredentialsRefused(self.instance.clone()))
                }
            }
            Err(error) => self.event_sender.send(Event::InstanceCredentialsFailed(
                self.instance.clone(),
                format!("{}", error),
            )),
        } {
            eprintln!(
                "Channel communication error during credential updater : {}",
                error
            )
        }
    }

    fn check_credentials(&self) -> Result<bool> {
        Ok(Client::new(
            self.instance.url(None),
            self.instance.username.clone(),
            self.instance.password.clone(),
        )
        .context("Construct http client")?
        .check_credentials()
        .context("Check credentials")?)
    }
}

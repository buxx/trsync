use reqwest::Method;

use crate::{
    error::ClientError,
    model::{Instance, Workspace},
};

const DEFAULT_CLIENT_TIMEOUT: u64 = 30;

pub struct Client {
    instance: Instance,
    client: reqwest::blocking::Client,
}

impl Client {
    pub fn new(instance: Instance) -> Self {
        Self {
            instance,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(DEFAULT_CLIENT_TIMEOUT))
                .build()
                .unwrap(),
        }
    }

    pub fn get_workspace(&self, workspace_id: u32) -> Result<Workspace, ClientError> {
        let url = self
            .instance
            .url(Some(&format!("/workspaces/{}", workspace_id)));
        log::debug!("Get workspace at url '{}'", url);
        let response = self
            .client
            .request(Method::GET, url)
            .basic_auth(&self.instance.username, Some(&self.instance.password))
            .send()?;

        let status_code = response.status().as_u16();
        match status_code {
            200 => Ok(response.json::<Workspace>()?),
            401 => Err(ClientError::Unauthorized),
            _ => {
                let text = response.text()?;
                Err(ClientError::UnexpectedResponse(format!(
                    "Unexpected response status '{}' during fetching workspace '{}' : '{}'",
                    status_code, workspace_id, text
                )))
            }
        }
    }
}

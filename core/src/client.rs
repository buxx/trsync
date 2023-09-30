use std::{path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use mockall::automock;
use reqwest::Method;
use serde_json::Value;
use thiserror::Error;

use crate::{
    content::Content,
    instance::{ContentFileName, ContentId, RevisionId, Workspace},
    types::ContentType,
    user::UserId,
};

const DEFAULT_CLIENT_TIMEOUT: u64 = 10;

#[derive(Debug, Clone, Error)]
pub enum TracimClientError {
    // Lister les erreurs en diff bien les erreurs ou on ne sais pas
    // (erreur reseau) des erreurs metier que l'on peut soit accepter soit rattraper
}

#[automock]
pub trait TracimClient {
    fn create_content(
        &self,
        file_name: ContentFileName,
        type_: ContentType,
        parent: Option<ContentId>,
    ) -> Result<ContentId>;
    fn set_label(&self, content_id: ContentId, value: ContentFileName) -> Result<RevisionId>;
    fn set_parent(&self, content_id: ContentId, value: Option<ContentId>) -> Result<RevisionId>;
    fn trash_content(&self, content_id: ContentId) -> Result<(), TracimClientError>;
    fn get_content(&self, content_id: ContentId) -> Result<Content, TracimClientError>;
    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        path: &PathBuf,
    ) -> Result<(), TracimClientError>;
    fn fill_content_with_file(
        &self,
        content_id: ContentId,
        path: &PathBuf,
    ) -> Result<(), TracimClientError>; // TODO : return new RevisionId
}

#[derive(Clone)]
pub struct Client {
    address: String,
    username: String,
    password: String,
    client: reqwest::blocking::Client,
}

impl Client {
    pub fn new(address: String, username: String, password: String) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_CLIENT_TIMEOUT))
            .build()?;
        Ok(Self {
            address,
            username,
            password,
            client,
        })
    }

    pub fn check_credentials(&self) -> Result<Option<UserId>> {
        let response = self
            .client
            .request(Method::GET, format!("{}/auth/whoami", self.address))
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()
            .context(format!(
                "Make authentication request for instance '{}'",
                self.address
            ))?;

        if response.status() == 200 {
            let user_id = response.json::<Value>()?["user_id"]
                .as_i64()
                .context("Read user_id property of response")? as i32;
            return Ok(Some(UserId(user_id)));
        }

        Ok(None)
    }

    pub fn workspaces(&self) -> Result<Vec<Workspace>> {
        let user_id = self
            .check_credentials()?
            .context("Get user user_id for grab workspaces")?;
        let response = self
            .client
            .request(
                Method::GET,
                format!("{}/users/{}/workspaces", self.address, user_id),
            )
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()
            .context(format!("Grab workspaces for instance '{}'", self.address))?;

        if response.status() == 200 {
            return response
                .json::<Vec<Workspace>>()
                .context("Read workspaces from response");
        }

        bail!("Response status code was '{}'", response.status())
    }
}

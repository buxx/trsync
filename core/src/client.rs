use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::Method;
use serde_json::Value;

use crate::{instance::Workspace, user::UserId};

const DEFAULT_CLIENT_TIMEOUT: u64 = 10;

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
            return Ok(response
                .json::<Vec<Workspace>>()
                .context("Read workspaces from response")?);
        }

        bail!("Response status code was '{}'", response.status())
    }
}

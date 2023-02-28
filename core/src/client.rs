use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Method;

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

    pub fn check_credentials(&self) -> Result<bool> {
        let response = self
            .client
            .request(Method::GET, format!("{}/auth/whoami", self.address))
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()
            .context("Make authentication request")?;

        Ok(response.status() == 200)
    }
}

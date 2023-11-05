use std::{io, path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use mockall::automock;
use reqwest::{
    blocking::{multipart, Response},
    Method,
};
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use strum_macros::Display;
use thiserror::Error;

use crate::{
    instance::{ContentFileName, ContentId, RevisionId, Workspace, WorkspaceId},
    types::ContentType,
    user::UserId,
};

pub const CONTENT_ALREADY_EXIST_ERR_CODE: u64 = 3002;
pub const DEFAULT_CLIENT_TIMEOUT: u64 = 30;

#[derive(Debug, Clone, Error, Display)]
pub enum TracimClientError {
    ContentAlreadyExist,
    Unknown,
}

impl TracimClientError {
    fn from_code(error_code: u64) -> TracimClientError {
        match error_code {
            CONTENT_ALREADY_EXIST_ERR_CODE => TracimClientError::ContentAlreadyExist,
            _ => TracimClientError::Unknown,
        }
    }
}

#[automock]
pub trait TracimClient {
    fn create_content(
        &self,
        file_name: ContentFileName,
        type_: ContentType,
        parent: Option<ContentId>,
    ) -> Result<ContentId>;
    fn set_label(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: ContentFileName,
    ) -> Result<RevisionId>;
    fn set_parent(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: Option<ContentId>,
    ) -> Result<RevisionId>;
    fn trash_content(&self, content_id: ContentId) -> Result<()>;
    fn get_content(&self, content_id: ContentId) -> Result<RemoteContent>;
    // FIXME BS NOW : Iterable
    fn get_contents(&self) -> Result<Vec<RemoteContent>>;
    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<()>;
    fn fill_content_with_file(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<RevisionId>; // TODO : return new RevisionId
    fn clone(&self) -> Box<dyn TracimClient>;
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteContent {
    pub content_id: ContentId,
    pub current_revision_id: RevisionId,
    pub parent_id: Option<i32>,
    pub content_type: String,
    pub modified: String,
    pub raw_content: Option<String>,
    pub filename: String,
    pub is_deleted: bool,
    pub is_archived: bool,
    pub sub_content_types: Vec<String>,
}

pub struct Tracim {
    base_address: String,
    workspace_id: WorkspaceId,
    client: reqwest::blocking::Client,
    username: String,
    password: String,
}

impl Tracim {
    pub fn new(
        base_address: String,
        workspace_id: WorkspaceId,
        client: reqwest::blocking::Client,
        username: String,
        password: String,
    ) -> Self {
        Self {
            base_address,
            workspace_id,
            client,
            username,
            password,
        }
    }

    pub fn workspace_url(&self, suffix: &str) -> String {
        format!(
            "{}workspaces/{}/{}",
            self.base_address, self.workspace_id, suffix
        )
    }

    fn created_content_id(&self, response: Response) -> Result<ContentId> {
        match response.status().as_u16() {
            200 => self.response_content_id(response),
            _ => bail!(self
                .response_error(response)
                .context("Interpret response error")?),
        }
    }

    fn created_revision_id(&self, response: Response) -> Result<RevisionId> {
        match response.status().as_u16() {
            200 => self.response_revision_id(response),
            _ => bail!(self
                .response_error(response)
                .context("Interpret response error")?),
        }
    }

    fn no_content_response(&self, response: Response) -> Result<()> {
        match response.status().as_u16() {
            204 => Ok(()),
            _ => bail!(self
                .response_error(response)
                .context("Interpret response error")?),
        }
    }

    fn response_content_id(&self, response: Response) -> Result<ContentId> {
        let value = response.json::<Value>()?;
        let data = value
            .as_object()
            .context(format!("Read response object : {:?}", value))?;
        let raw_content_id = data["content_id"]
            .as_i64()
            .context(format!("Read content_id from response : {:?}", data))?;
        Ok(ContentId(raw_content_id as i32))
    }

    fn response_revision_id(&self, response: Response) -> Result<RevisionId> {
        let value = response.json::<Value>()?;
        let data = value
            .as_object()
            .context(format!("Read response object : {:?}", value))?;
        let raw_revision_id = data["revision_id"]
            .as_i64()
            .context(format!("Read revision_id from response : {:?}", data))?;
        Ok(RevisionId(raw_revision_id as i32))
    }

    fn response_error(&self, response: Response) -> Result<TracimClientError> {
        let error_code = response.json::<Value>()?["code"]
            .as_u64()
            .context("Read error code from response")?;

        Ok(TracimClientError::from_code(error_code))
    }

    fn create_folder(
        &self,
        file_name: ContentFileName,
        parent: Option<ContentId>,
    ) -> Result<ContentId> {
        let url = self.workspace_url("contents");
        let mut data = Map::new();
        data.insert(
            "content_type".to_string(),
            json!(ContentType::Folder.to_string()),
        );
        data.insert("label".to_string(), json!(file_name.0));
        if let Some(parent_id) = parent {
            data.insert("parent_id".to_string(), json!(parent_id.0));
        };
        let response = self
            .client
            .request(Method::POST, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .json(&data)
            .send()
            .context("Post created folder request")?;

        self.created_content_id(response)
            .context("Read post created folder request response")
    }

    fn update_content(
        &self,
        content_id: ContentId,
        type_: ContentType,
        data: Map<String, Value>,
    ) -> Result<RevisionId> {
        let url = format!("{}/{}", type_.url_prefix(), content_id);
        let mut data = data.clone();

        // Be compatible with Tracim which not have this https://github.com/tracim/tracim/pull/5864
        if type_ == ContentType::Folder {
            let remote_content = self.get_content(content_id)?;
            data.insert(
                "sub_content_types".to_string(),
                json!(remote_content.sub_content_types),
            );
        }

        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .json(&data)
            .send()?;

        self.created_revision_id(response)
            .context("Read post created revision request response")
    }

    fn create_file(
        &self,
        _file_name: ContentFileName,
        _parent: Option<ContentId>,
    ) -> Result<ContentId> {
        todo!()
    }

    fn fill_content_file_with_file(
        &self,
        content_id: ContentId,
        path: &PathBuf,
    ) -> Result<RevisionId> {
        let form = multipart::Form::new()
            .file("files", &path)
            .context(format!("Prepare upload form for {}", path.display()))?;
        let file_name = path
            .file_name()
            .context(format!("Determine file name of {}", path.display()))?
            .to_str()
            .context(format!("Determine file name of {}", path.display()))?;
        let url = self.workspace_url(&format!("files/{}/raw/{}", content_id, file_name));

        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .multipart(form)
            .send()?;

        self.created_revision_id(response)
    }
}

impl TracimClient for Tracim {
    fn create_content(
        &self,
        file_name: ContentFileName,
        type_: ContentType,
        parent: Option<ContentId>,
    ) -> Result<ContentId> {
        match type_ {
            ContentType::Folder => self.create_folder(file_name, parent),
            ContentType::HtmlDocument => todo!(),
            ContentType::File => self.create_file(file_name, parent),
        }
    }

    fn set_label(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: ContentFileName,
    ) -> Result<RevisionId> {
        let label = value.label(&type_);
        let mut data = Map::new();
        data.insert("label".to_string(), json!(label));
        data.insert("file_name".to_string(), json!(value.0));

        self.update_content(content_id, type_, data)
    }

    fn set_parent(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: Option<ContentId>,
    ) -> Result<RevisionId> {
        let mut data = Map::new();
        data.insert("parent_id".to_string(), json!(value));
        self.update_content(content_id, type_, data)
    }

    fn trash_content(&self, content_id: ContentId) -> Result<()> {
        let url = self.workspace_url(&format!("contents/{}/trashed", content_id));
        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        self.no_content_response(response)
            .context("Read post created revision request response")
    }

    fn get_content(&self, content_id: ContentId) -> Result<RemoteContent> {
        let response = self
            .client
            .request(
                Method::GET,
                self.workspace_url(&format!("contents/{}", content_id)),
            )
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        Ok(response.json::<RemoteContent>()?)
    }

    fn get_contents(&self) -> Result<Vec<RemoteContent>> {
        let url = self.workspace_url("contents");

        let response = self
            .client
            .request(Method::GET, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        let status_code = response.status().as_u16();
        match status_code {
            200 => Ok(response
                .json::<Paginated<Vec<RemoteContent>>>()?
                .items
                .into_iter()
                .filter(|c| ContentType::from_str(c.content_type.as_str()).is_some())
                .collect::<Vec<RemoteContent>>()),
            _ => {
                bail!(self
                    .response_error(response)
                    .context("Interpret response error after get all remote contents")?)
            }
        }
    }

    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        _type_: ContentType,
        path: &PathBuf,
    ) -> Result<()> {
        // FIXME BS NOW: html-doc
        let mut response = self
            .client
            .request(
                Method::GET,
                self.workspace_url(&format!("files/{}/raw/_", content_id)),
            )
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        let mut out = std::fs::File::create(path)
            .context(format!("Open or create file at {}", path.display()))?;
        io::copy(&mut response, &mut out).context(format!("Fill file at {}", path.display()))?;

        Ok(())
    }

    fn fill_content_with_file(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<RevisionId> {
        match type_ {
            ContentType::File => self.fill_content_file_with_file(content_id, path),
            ContentType::Folder => todo!(),
            ContentType::HtmlDocument => todo!(),
        }
    }

    fn clone(&self) -> Box<dyn TracimClient> {
        Box::new(Tracim {
            base_address: self.base_address.clone(),
            workspace_id: self.workspace_id,
            client: self.client.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Paginated<T> {
    has_next: bool,
    has_previous: bool,
    items: T,
    next_page_token: String,
    per_page: i32,
    previous_page_token: String,
}

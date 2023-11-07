use std::{io, path::PathBuf, time::Duration};

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
    ConnectionError,
    TimeoutError,
    Unknown(String),
    PrepareError(String),
    InvalidResponse(String, Value),
    AuthenticationError,
}

impl TracimClientError {
    fn from_code(error_code: u64) -> Option<TracimClientError> {
        match error_code {
            CONTENT_ALREADY_EXIST_ERR_CODE => Some(TracimClientError::ContentAlreadyExist),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for TracimClientError {
    fn from(error: reqwest::Error) -> Self {
        // FIXME BS NOW : TrSync must switch into offline mode or something
        if error.is_connect() {
            return Self::ConnectionError;
        }

        // FIXME BS NOW : retry ?
        if error.is_timeout() {
            return Self::TimeoutError;
        }

        Self::Unknown(error.to_string())
    }
}

impl From<anyhow::Error> for TracimClientError {
    fn from(error: anyhow::Error) -> Self {
        TracimClientError::Unknown(format!("{:#}", error))
    }
}

#[derive(Debug)]
pub enum ParentIdParameter {
    Root,
    Some(ContentId),
}

impl ParentIdParameter {
    pub fn from_value(value: Option<ContentId>) -> Self {
        match value {
            Some(content_id) => Self::Some(content_id),
            None => Self::Root,
        }
    }

    pub fn to_parameter_value(&self) -> i32 {
        match self {
            ParentIdParameter::Root => 0,
            ParentIdParameter::Some(parent_id) => parent_id.0,
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
        path: &PathBuf,
    ) -> Result<ContentId, TracimClientError>;
    fn set_label(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: ContentFileName,
    ) -> Result<RevisionId, TracimClientError>;
    fn set_parent(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: Option<ContentId>,
    ) -> Result<RevisionId, TracimClientError>;
    fn trash_content(&self, content_id: ContentId) -> Result<(), TracimClientError>;
    fn get_content(&self, content_id: ContentId) -> Result<RemoteContent, TracimClientError>;
    fn find_one(
        &self,
        file_name: &ContentFileName,
        parent_id: ParentIdParameter,
    ) -> Result<Option<ContentId>, TracimClientError>;
    // FIXME BS NOW : Iterable
    fn get_contents(&self) -> Result<Vec<RemoteContent>, TracimClientError>;
    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<(), TracimClientError>;
    fn fill_content_with_file(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<RevisionId, TracimClientError>; // TODO : return new RevisionId
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
    pub fn new(
        address: String,
        username: String,
        password: String,
    ) -> Result<Self, TracimClientError> {
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

    pub fn check_credentials(&self) -> Result<Option<UserId>, TracimClientError> {
        let response = self
            .client
            .request(Method::GET, format!("{}/auth/whoami", self.address))
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        if response.status() == 200 {
            let response_value = response.json::<Value>()?;
            let user_id =
                response_value["user_id"]
                    .as_i64()
                    .ok_or(TracimClientError::InvalidResponse(
                        "Response user_id seems not be an integer".to_string(),
                        response_value.clone(),
                    ))? as i32;
            return Ok(Some(UserId(user_id)));
        }

        Ok(None)
    }

    pub fn workspaces(&self) -> Result<Vec<Workspace>, TracimClientError> {
        let user_id = self
            .check_credentials()?
            .ok_or(TracimClientError::AuthenticationError)?;
        let response = self
            .client
            .request(
                Method::GET,
                format!("{}/users/{}/workspaces", self.address, user_id),
            )
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        if response.status() == 200 {
            return Ok(response.json::<Vec<Workspace>>()?);
        }

        // TODO : detail on error (if this code is used !)
        Err(TracimClientError::Unknown(format!(
            "Response status code is {}",
            response.status()
        )))
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

    fn created_content_id(&self, response: Response) -> Result<ContentId, TracimClientError> {
        match response.status().as_u16() {
            200 => self.response_content_id(response),
            _ => Err(self.response_error(response)?),
        }
    }

    fn created_revision_id(&self, response: Response) -> Result<RevisionId, TracimClientError> {
        match response.status().as_u16() {
            200 => self.response_revision_id(response),
            _ => Err(self.response_error(response)?),
        }
    }

    fn no_content(&self, response: Response) -> Result<(), TracimClientError> {
        match response.status().as_u16() {
            204 => Ok(()),
            _ => Err(self.response_error(response)?),
        }
    }

    fn no_content_response(&self, response: Response) -> Result<(), TracimClientError> {
        match response.status().as_u16() {
            204 => Ok(()),
            _ => Err(self.response_error(response)?),
        }
    }

    fn response_content_id(&self, response: Response) -> Result<ContentId, TracimClientError> {
        let value = response.json::<Value>()?;
        let data = value.as_object().ok_or(TracimClientError::InvalidResponse(
            "Response body is not an object".to_string(),
            value.clone(),
        ))?;
        let raw_content_id =
            data["content_id"]
                .as_i64()
                .ok_or(TracimClientError::InvalidResponse(
                    "Response content_id is not an integer".to_string(),
                    data["content_id"].clone(),
                ))?;
        Ok(ContentId(raw_content_id as i32))
    }

    fn response_revision_id(&self, response: Response) -> Result<RevisionId, TracimClientError> {
        let value = response.json::<Value>()?;
        let data = value.as_object().ok_or(TracimClientError::InvalidResponse(
            "Response body is not an object".to_string(),
            value.clone(),
        ))?;
        let raw_revision_id =
            data["revision_id"]
                .as_i64()
                .ok_or(TracimClientError::InvalidResponse(
                    "Response revision_id is not an integer".to_string(),
                    data["revision_id"].clone(),
                ))?;
        Ok(RevisionId(raw_revision_id as i32))
    }

    fn response_error(&self, response: Response) -> Result<TracimClientError, TracimClientError> {
        let content_value = response.json::<Value>()?;
        dbg!(&content_value);
        let error_code =
            content_value["code"]
                .as_u64()
                .ok_or(TracimClientError::InvalidResponse(
                    "Response code is not an integer".to_string(),
                    content_value["code"].clone(),
                ))?;

        if let Some(error) = TracimClientError::from_code(error_code) {
            dbg!(&error);
            return Ok(error);
        }

        if let Some(message) = content_value["message"].as_str() {
            return Ok(TracimClientError::Unknown(message.to_string()));
        }

        Ok(TracimClientError::Unknown("Unknown error".to_string()))
    }

    fn create_folder(
        &self,
        file_name: ContentFileName,
        parent: Option<ContentId>,
    ) -> Result<ContentId, TracimClientError> {
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
            .send()?;

        self.created_content_id(response)
    }

    fn update_content(
        &self,
        content_id: ContentId,
        type_: ContentType,
        data: Map<String, Value>,
    ) -> Result<RevisionId, TracimClientError> {
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
    }

    fn create_file(
        &self,
        parent: Option<ContentId>,
        path: &PathBuf,
    ) -> Result<ContentId, TracimClientError> {
        let mut form = multipart::Form::new();
        if let Some(parent_id) = parent {
            form = form.text("parent_id", parent_id.to_string());
        };
        let url = self.workspace_url("files");
        form = form.file("files", path).map_err(|e| {
            TracimClientError::PrepareError(format!(
                "Error during preparation of form for file {} : {}",
                path.display(),
                e
            ))
        })?;

        let response = self
            .client
            .request(Method::POST, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .multipart(form)
            .send()?;

        self.created_content_id(response)
    }

    fn fill_content_file_with_file(
        &self,
        content_id: ContentId,
        path: &PathBuf,
    ) -> Result<RevisionId, TracimClientError> {
        let form = multipart::Form::new()
            .file("files", path)
            .map_err(|error| {
                TracimClientError::PrepareError(format!(
                    "Error during preparation of form for file {} : {}",
                    path.display(),
                    error
                ))
            })?;
        let file_name = path
            .file_name()
            .ok_or(TracimClientError::PrepareError(format!(
                "Determine file name of {}",
                path.display()
            )))?
            .to_str()
            .ok_or(TracimClientError::PrepareError(format!(
                "Determine file name of {}",
                path.display()
            )))?;
        let url = self.workspace_url(&format!("files/{}/raw/{}", content_id, file_name));

        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .multipart(form)
            .send()?;

        self.no_content(response)?;
        let content = self.get_content(content_id)?;
        Ok(content.current_revision_id)
    }
}

impl TracimClient for Tracim {
    fn create_content(
        &self,
        file_name: ContentFileName,
        type_: ContentType,
        parent: Option<ContentId>,
        path: &PathBuf,
    ) -> Result<ContentId, TracimClientError> {
        match type_ {
            ContentType::Folder => self.create_folder(file_name, parent),
            ContentType::HtmlDocument => todo!(),
            ContentType::File => self.create_file(parent, path),
        }
    }

    fn set_label(
        &self,
        content_id: ContentId,
        type_: ContentType,
        value: ContentFileName,
    ) -> Result<RevisionId, TracimClientError> {
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
    ) -> Result<RevisionId, TracimClientError> {
        let mut data = Map::new();
        data.insert("parent_id".to_string(), json!(value));
        self.update_content(content_id, type_, data)
    }

    fn trash_content(&self, content_id: ContentId) -> Result<(), TracimClientError> {
        let url = self.workspace_url(&format!("contents/{}/trashed", content_id));
        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        self.no_content_response(response)
    }

    fn get_content(&self, content_id: ContentId) -> Result<RemoteContent, TracimClientError> {
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

    fn get_contents(&self) -> Result<Vec<RemoteContent>, TracimClientError> {
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
            _ => Err(self.response_error(response)?),
        }
    }

    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        _type_: ContentType,
        path: &PathBuf,
    ) -> Result<(), TracimClientError> {
        // FIXME BS NOW: html-doc
        let mut response = self
            .client
            .request(
                Method::GET,
                self.workspace_url(&format!("files/{}/raw/_", content_id)),
            )
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        let mut out = std::fs::File::create(path).map_err(|error| {
            TracimClientError::PrepareError(format!(
                "Error when open or create file at {}: {}",
                path.display(),
                error
            ))
        })?;
        io::copy(&mut response, &mut out).map_err(|error| {
            TracimClientError::PrepareError(format!(
                "Error when fill file at {}: {}",
                path.display(),
                error
            ))
        })?;

        Ok(())
    }

    fn fill_content_with_file(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<RevisionId, TracimClientError> {
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

    fn find_one(
        &self,
        file_name: &ContentFileName,
        parent_id: ParentIdParameter,
    ) -> Result<Option<ContentId>, TracimClientError> {
        let url = self.workspace_url(&format!(
            "contents?parent_ids={}",
            parent_id.to_parameter_value()
        ));
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
                .find(|c| c.filename == file_name.0)
                .map(|c| c.content_id)),
            _ => Err(self.response_error(response)?),
        }
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

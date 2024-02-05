use std::{fs, io, path::PathBuf, time::Duration};

use mockall::automock;
use reqwest::{
    blocking::{multipart, Response},
    Method,
};
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::str::FromStr;
use thiserror::Error;

use crate::{
    instance::{ContentFileName, ContentId, RevisionId, Workspace, WorkspaceId},
    types::ContentType,
    user::UserId,
    utils::extract_html_body,
    HTML_DOCUMENT_LOCAL_EXTENSION,
};

pub const CONTENT_ALREADY_EXIST_ERR_CODE: u64 = 3002;
pub const CONTENT_IN_NOT_EDITABLE_STATE_ERR_CODE: u64 = 2044;
pub const CONTENT_NOT_FOUND: u64 = 1003;
pub const DEFAULT_CLIENT_TIMEOUT: u64 = 30;

#[derive(Debug, Clone, Error)]
pub enum TracimClientError {
    #[error("Content not found")]
    ContentNotFound,
    #[error("Content already exist")]
    ContentAlreadyExist,
    #[error("Content is deleted or archived")]
    ContentDeletedOrArchived,
    #[error("Connection error")]
    ConnectionError,
    #[error("Timeout error")]
    TimeoutError,
    #[error("Unknown error: `{0}`")]
    Unknown(String),
    #[error("Preparation error: `{0}`")]
    PrepareError(String),
    #[error("Invalid response: `{0}` (`{1}`)")]
    InvalidResponse(String, Value),
    #[error("Authentication error")]
    AuthenticationError,
    #[error("File {0} not found: {1}")]
    FileNotFound(PathBuf, String),
}

impl TracimClientError {
    fn from_code(error_code: u64) -> Option<TracimClientError> {
        match error_code {
            CONTENT_NOT_FOUND => Some(TracimClientError::ContentNotFound),
            CONTENT_ALREADY_EXIST_ERR_CODE => Some(TracimClientError::ContentAlreadyExist),
            CONTENT_IN_NOT_EDITABLE_STATE_ERR_CODE => {
                Some(TracimClientError::ContentDeletedOrArchived)
            }
            _ => None,
        }
    }
}

impl From<reqwest::Error> for TracimClientError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_connect() {
            return Self::ConnectionError;
        }

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

impl From<Option<ContentId>> for ParentIdParameter {
    fn from(value: Option<ContentId>) -> Self {
        match value {
            Some(value) => Self::Some(value),
            None => Self::Root,
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
        path: PathBuf,
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
        value: Option<ContentId>,
        new_workspace_id: Option<WorkspaceId>,
    ) -> Result<RevisionId, TracimClientError>;
    fn trash_content(&self, content_id: ContentId) -> Result<(), TracimClientError>;
    fn restore_content(&self, content_id: ContentId) -> Result<(), TracimClientError>;
    fn get_content(&self, content_id: ContentId) -> Result<RemoteContent, TracimClientError>;
    fn get_content_path(&self, content_id: ContentId) -> Result<PathBuf, TracimClientError>;
    fn find_one(
        &self,
        file_name: &ContentFileName,
        parent_id: ParentIdParameter,
    ) -> Result<Option<ContentId>, TracimClientError>;
    // TODO : Iterable
    fn get_contents(&self) -> Result<Vec<RemoteContent>, TracimClientError>;
    #[allow(clippy::ptr_arg)]
    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<(), TracimClientError>;
    #[allow(clippy::ptr_arg)]
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
            data["current_revision_id"]
                .as_i64()
                .ok_or(TracimClientError::InvalidResponse(
                    "Response current_revision_id is not an integer".to_string(),
                    data["current_revision_id"].clone(),
                ))?;
        Ok(RevisionId(raw_revision_id as i32))
    }

    fn response_error(&self, response: Response) -> Result<TracimClientError, TracimClientError> {
        let content_value = response.json::<Value>()?;
        let error_code =
            content_value["code"]
                .as_u64()
                .ok_or(TracimClientError::InvalidResponse(
                    "Response code is not an integer".to_string(),
                    content_value["code"].clone(),
                ))?;

        if let Some(error) = TracimClientError::from_code(error_code) {
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
        let url = self.workspace_url(&format!("{}/{}", type_.url_prefix(), content_id));
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

    fn create_note(
        &self,
        file_name: ContentFileName,
        parent: Option<ContentId>,
    ) -> Result<ContentId, TracimClientError> {
        let url = self.workspace_url("contents");
        let mut data = Map::new();

        let file_name = file_name.0.replace(HTML_DOCUMENT_LOCAL_EXTENSION, "");
        data.insert("label".to_string(), json!(file_name));
        data.insert("raw_content".to_string(), json!("".to_string()));
        data.insert(
            "content_type".to_string(),
            json!(ContentType::HtmlDocument.to_string()),
        );
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

    fn create_file(
        &self,
        parent: Option<ContentId>,
        path: PathBuf,
    ) -> Result<ContentId, TracimClientError> {
        let mut form = multipart::Form::new();
        if let Some(parent_id) = parent {
            form = form.text("parent_id", parent_id.to_string());
        };
        let url = self.workspace_url("files");
        form = form.file("files", &path).map_err(|e| {
            TracimClientError::PrepareError(format!(
                "Error during preparation of form for file {} : {}",
                path.display(),
                e
            ))
        })?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()?;
        let response = client
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

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()?;
        let response = client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .multipart(form)
            .send()?;

        self.no_content(response)?;
        let content = self.get_content(content_id)?;
        Ok(content.current_revision_id)
    }

    fn fill_content_note_with_file(
        &self,
        content_id: ContentId,
        path: &PathBuf,
    ) -> Result<RevisionId, TracimClientError> {
        let mut data = Map::new();
        let url = self.workspace_url(&format!("html-documents/{}", content_id));

        let html_content = fs::read_to_string(path)
            .map_err(|error| TracimClientError::FileNotFound(path.clone(), error.to_string()))?;
        let html_part_content: String = extract_html_body(&html_content).unwrap_or_else(|error| {
            log::debug!("Unable to extract html body content : '{}'", error);
            html_content.to_string()
        });
        data.insert("raw_content".to_string(), json!(html_part_content));

        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .json(&data)
            .send()?;

        self.created_revision_id(response)
    }

    pub async fn get_user_live_messages_response(
        &self,
        user_id: i32,
    ) -> Result<reqwest::Response, TracimClientError> {
        let url = format!("{}users/{}/live_messages", self.base_address, user_id);
        let response = reqwest::Client::new()
            .request(Method::GET, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()
            .await?;
        let response_status_code = response.status().as_u16();
        match response_status_code {
            200 => Ok(response),
            _ => {
                let text = response.text().await?;
                Err(TracimClientError::Unknown(format!(
                    "Unexpected response status {} : '{}'",
                    response_status_code, text,
                )))
            }
        }
    }

    pub fn get_user_id(&self) -> Result<i32, TracimClientError> {
        let url = format!("{}auth/whoami", self.base_address);
        let response = self
            .client
            .request(Method::GET, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .send()?;

        let response_status_code = response.status().as_u16();
        match response_status_code {
            200 => {
                let value = &response.json::<Value>()?;
                let data = value.as_object().ok_or(TracimClientError::InvalidResponse(
                    "Response content not appear to be an object".to_string(),
                    value.clone(),
                ))?;
                let user_id =
                    data["user_id"]
                        .as_i64()
                        .ok_or(TracimClientError::InvalidResponse(
                            "Response content object do not contains a integer user_id".to_string(),
                            data["user_id"].clone(),
                        ))?;
                Ok(user_id as i32)
            }
            _ => {
                let text = response.text()?;
                Err(TracimClientError::Unknown(format!(
                    "Unexpected response status {} : '{}'",
                    response_status_code, text,
                )))
            }
        }
    }
}

impl TracimClient for Tracim {
    fn create_content(
        &self,
        file_name: ContentFileName,
        type_: ContentType,
        parent: Option<ContentId>,
        path: PathBuf,
    ) -> Result<ContentId, TracimClientError> {
        match type_ {
            ContentType::Folder => self.create_folder(file_name, parent),
            ContentType::HtmlDocument => self.create_note(file_name, parent),
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
        value: Option<ContentId>,
        new_workspace_id: Option<WorkspaceId>,
    ) -> Result<RevisionId, TracimClientError> {
        let url = self.workspace_url(&format!("contents/{}/move", content_id));
        let mut data = Map::new();
        data.insert("new_parent_id".to_string(), json!(value));
        data.insert(
            "new_workspace_id".to_string(),
            json!(new_workspace_id.unwrap_or(self.workspace_id)),
        );

        let response = self
            .client
            .request(Method::PUT, url)
            .basic_auth(self.username.clone(), Some(self.password.clone()))
            .json(&data)
            .send()?;

        self.created_revision_id(response)
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

    fn restore_content(&self, content_id: ContentId) -> Result<(), TracimClientError> {
        let url = self.workspace_url(&format!("contents/{}/trashed/restore", content_id));
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

        let status_code = response.status().as_u16();
        match status_code {
            200 => Ok(response.json::<RemoteContent>()?),
            _ => Err(self.response_error(response)?),
        }
    }

    fn get_content_path(&self, content_id: ContentId) -> Result<PathBuf, TracimClientError> {
        let mut path = PathBuf::new();
        let mut content = self.get_content(content_id)?;
        let mut reversed_content_file_names = vec![content.filename.clone()];

        while let Some(parent_id) = content.parent_id {
            content = self.get_content(ContentId(parent_id))?;
            reversed_content_file_names.push(content.filename.clone());
        }

        let content_file_names: Vec<String> =
            reversed_content_file_names.into_iter().rev().collect();
        path.extend(content_file_names);

        Ok(path)
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
                .filter(|c| ContentType::from_str(c.content_type.as_str()).is_ok())
                .collect::<Vec<RemoteContent>>()),
            _ => Err(self.response_error(response)?),
        }
    }

    fn fill_file_with_content(
        &self,
        content_id: ContentId,
        type_: ContentType,
        path: &PathBuf,
    ) -> Result<(), TracimClientError> {
        match type_ {
            ContentType::File => {
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
            }
            ContentType::HtmlDocument => {
                let content = self.get_content(content_id)?;
                std::fs::write(path, content.raw_content.unwrap_or("".to_string())).map_err(
                    |error| {
                        TracimClientError::PrepareError(format!(
                            "Error when try to write file at {}: {}",
                            path.display(),
                            error
                        ))
                    },
                )?;
            }
            ContentType::Folder => {}
        };

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
            ContentType::HtmlDocument => self.fill_content_note_with_file(content_id, path),
            ContentType::Folder => unreachable!(),
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
                .filter(|c| ContentType::from_str(c.content_type.as_str()).is_ok())
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

use std::path::Path;

use reqwest::blocking::{multipart, Response};
use reqwest::Method;

use serde_json::Value;

use crate::error::ClientError;
use crate::types::RevisionId;
use crate::{
    remote::RemoteContent,
    types::{ContentId, ContentType},
};

const CONTENT_ALREADY_EXIST_ERR_CODE: u16 = 3002;
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
            ParentIdParameter::Some(parent_id) => *parent_id,
        }
    }
}

pub struct Client {
    tracim_api_key: String,
    tracim_user_name: String,
    client: reqwest::blocking::Client,
}

impl Client {
    pub fn new(tracim_api_key: String, tracim_user_name: String) -> Self {
        Self {
            tracim_api_key,
            tracim_user_name,
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn create_content(
        &self,
        absolute_file_path: String,
        content_type: ContentType,
        parent_content_id: Option<ContentId>,
    ) -> Result<(ContentId, RevisionId), ClientError> {
        let mut form = multipart::Form::new();
        if let Some(parent_content_id) = parent_content_id {
            form = form.text("parent_id", parent_content_id.to_string());
        };

        let mut url = "".to_string();
        if content_type == ContentType::Folder {
            url = format!("https://tracim.bux.fr/api/workspaces/4/contents");
            form = form.text("content_type", ContentType::Folder.to_string());
        } else {
            url = "https://tracim.bux.fr/api/workspaces/4/files".to_string();
            form = match form.file("files", &absolute_file_path) {
                Ok(form) => form,
                Err(err) => {
                    return Err(ClientError::InputFileError(format!(
                        "{}: {:?}",
                        absolute_file_path, err
                    )))
                }
            };
        }

        let response = self
            .client
            .request(Method::POST, url)
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .multipart(form)
            .send()?;

        match &response.status().as_u16() {
            200 => {
                let content_id = response.json::<Value>().unwrap().as_object().unwrap()
                    ["content_id"]
                    .as_i64()
                    .unwrap() as ContentId;
                let revision_id = response.json::<Value>().unwrap().as_object().unwrap()
                    ["revision_id"]
                    .as_i64()
                    .unwrap() as RevisionId;
                Ok((content_id, revision_id))
            }
            400 => {
                let error_code = match response.json::<Value>()?["code"].as_u64() {
                    Some(code) => code as u16,
                    None => {
                        return Err(ClientError::AlreadyExistResponseAndFailToFoundIt(
                            "Fail when trying to determine response error code".to_string(),
                        ))
                    }
                };
                match error_code {
                    CONTENT_ALREADY_EXIST_ERR_CODE => {
                        match self.find_existing(absolute_file_path, parent_content_id) {
                            Ok((found_content_id, found_revision_id)) => {
                                Err(ClientError::AlreadyExistResponse(
                                    found_content_id,
                                    found_revision_id,
                                ))
                            }
                            Err(err) => {
                                Err(ClientError::AlreadyExistResponseAndFailToFoundIt(format!(
                                    "Error when trying to found already existing content : {}",
                                    err
                                )))
                            }
                        }
                    }
                    _ => Err(ClientError::AlreadyExistResponseAndFailToFoundIt(format!(
                        "Response error code was {}",
                        error_code
                    ))),
                }
            }
            _ => Err(ClientError::UnexpectedResponse(format!(
                "Response status was {}",
                &response.status()
            ))),
        }
    }

    fn find_existing(
        &self,
        absolute_file_path: String,
        parent_id: Option<ContentId>,
    ) -> Result<(ContentId, RevisionId), ClientError> {
        let file_name = match Path::new(&absolute_file_path).file_name() {
            Some(file_name) => match file_name.to_str() {
                Some(file_name) => file_name.to_string(),
                None => return Err(ClientError::RequestError(format!(
                    "Given absolute file path '{}' produce error when trying to get file_name String version",
                    absolute_file_path
                ))),
            },
            None => {
                return Err(ClientError::RequestError(format!(
                    "Given absolute file path {} doesn't permit to determine file_name",
                    absolute_file_path
                )))
            }
        };
        for remote_content in
            self.get_remote_contents(Some(ParentIdParameter::from_value(parent_id)))?
        {
            if remote_content.filename == file_name {
                return Ok((remote_content.content_id, remote_content.revision_id));
            }
        }
        Err(ClientError::NotFoundResponse(
            "Didn't find matching content filename".to_string(),
        ))
    }

    pub fn update_content(
        &self,
        absolute_file_path: String,
        file_name: String,
        content_type: ContentType,
        content_id: ContentId,
    ) -> Result<RevisionId, ClientError> {
        println!(
            "Update remote content {} with file {}",
            content_id, absolute_file_path
        );

        let mut url = "".to_string();
        let mut form = multipart::Form::new();
        if content_type == ContentType::Folder {
            let content = self.get_remote_content(content_id)?;
            return Ok(content.revision_id);
        } else {
            form = match form.file("files", &absolute_file_path) {
                Ok(form) => form,
                Err(err) => {
                    return Err(ClientError::InputFileError(format!(
                        "{}: {:?}",
                        absolute_file_path, err
                    )))
                }
            };
            url = format!(
                "https://tracim.bux.fr/api/workspaces/4/files/{}/raw/{}",
                content_id, file_name,
            );
        };

        let response = self
            .client
            .request(Method::PUT, url)
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .multipart(form)
            .send()?;
        match response.status().as_u16() {
            200 => {
                let content = self.get_remote_content(content_id)?;
                Ok(content.revision_id)
            }
            _ => Err(ClientError::UnexpectedResponse(format!(
                "Response status code was {}",
                response.status().as_u16(),
            ))),
        }
    }

    pub fn trash_content(&self, content_id: ContentId) -> Result<(), ClientError> {
        let response = self
            .client
            .request(
                Method::PUT,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/contents/{}/trashed",
                    content_id,
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()?;

        match response.status().as_u16() {
            204 => Ok(()),
            _ => Err(ClientError::UnexpectedResponse(format!(
                "Response status code was {}",
                response.status().as_u16(),
            ))),
        }
    }

    pub fn get_remote_content(&self, content_id: ContentId) -> Result<RemoteContent, ClientError> {
        let response = self
            .client
            .request(
                Method::GET,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/contents/{}",
                    content_id
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()?;

        Ok(response.json::<RemoteContent>()?)
    }

    pub fn build_relative_path(&self, content: &RemoteContent) -> Result<String, ClientError> {
        if let Some(parent_id) = content.parent_id {
            let mut path_parts: Vec<String> = vec![content.filename.clone()];
            let mut last_seen_parent_id = parent_id;
            loop {
                let response = self
                    .client
                    .request(
                        Method::GET,
                        format!(
                            "https://tracim.bux.fr/api/workspaces/4/folders/{}",
                            last_seen_parent_id
                        ),
                    )
                    .header("Tracim-Api-Key", &self.tracim_api_key)
                    .header("Tracim-Api-Login", &self.tracim_user_name)
                    .send()?;

                match response.status().as_u16() {
                    200 => {},
                    _ => {
                        return Err(ClientError::UnexpectedResponse(format!(
                            "Fail to build relative path for content id {}, response status code was {}",
                            content.content_id, response.status().as_u16(),
                        )))
                    }
                };

                let folder = response.json::<RemoteContent>()?;
                path_parts.push(folder.filename);
                if let Some(folder_parent_id) = folder.parent_id {
                    last_seen_parent_id = folder_parent_id;
                } else {
                    // TODO : this is very ugly code !
                    let mut relative_path_string = "".to_string();
                    for path_part in path_parts.iter().rev() {
                        let relative_path = Path::new(&relative_path_string).join(path_part);
                        relative_path_string = match relative_path.to_str() {
                            Some(relative_path_string) => relative_path_string.to_string(),
                            None => {
                                return Err(ClientError::RequestError(format!(
                                    "Fail to convert {:?}, to String",
                                    relative_path,
                                )))
                            }
                        };
                    }
                    return Ok(relative_path_string);
                }
            }
        } else {
            Ok(content.filename.clone())
        }
    }

    pub fn get_file_content_response(
        &self,
        content_id: ContentId,
        file_name: String,
    ) -> Result<Response, ClientError> {
        Ok(self
            .client
            .request(
                Method::GET,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/files/{}/raw/{}",
                    content_id, file_name,
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()?)
    }

    pub fn get_remote_contents(
        &self,
        parent_id: Option<ParentIdParameter>,
    ) -> Result<Vec<RemoteContent>, ClientError> {
        let url = match parent_id {
            Some(parent_id) => format!(
                "https://tracim.bux.fr/api/workspaces/4/contents?parent_ids={}",
                parent_id.to_parameter_value()
            ),
            None => "https://tracim.bux.fr/api/workspaces/4/contents".to_string(),
        };

        Ok(self
            .client
            .request(Method::GET, url)
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()?
            .json::<Vec<RemoteContent>>()?
            .into_iter()
            .filter(|c| ContentType::from_str(&c.content_type.as_str()).is_some())
            .collect::<Vec<RemoteContent>>())
    }
}

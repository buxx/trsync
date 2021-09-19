use std::path::Path;

use reqwest::blocking::{multipart, Response};
use reqwest::Method;

use serde_json::Value;

use crate::error::ClientError;
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
        file_name: String,
        content_type: ContentType,
        parent_content_id: Option<ContentId>,
    ) -> Result<ContentId, ClientError> {
        // TODO : manage folders too

        let mut form = multipart::Form::new();
        if let Some(parent_content_id) = parent_content_id {
            form = form.text("parent_id", parent_content_id.to_string());
        };
        form = match form.file("files", absolute_file_path) {
            Ok(form) => form,
            Err(err) => {
                return Err(ClientError::InputFileError(format!(
                    "{}: {:?}",
                    absolute_file_path, err
                )))
            }
        };

        // TODO : need to check if response is 200 !!
        let response = self
            .client
            .request(Method::POST, "https://tracim.bux.fr/api/workspaces/4/files")
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .multipart(form)
            .send()?;

        match &response.status().as_u16() {
            200 => Ok(
                response.json::<Value>().unwrap().as_object().unwrap()["content_id"]
                    .as_i64()
                    .unwrap() as ContentId,
            ),
            400 => {
                match self.get_error_code(&response) {
                    Ok(error_code) => {
                        match error_code {
                            CONTENT_ALREADY_EXIST_ERR_CODE => {
                                match self.find_existing(absolute_file_path, parent_content_id) {
                                Ok(found_content_id) => Err(ClientError::AlreadyExistResponse(found_content_id)),
                                Err(err) => Err(ClientError::AlreadyExistResponseAndFailToFoundIt(format!("Error when trying to found already existing content : {}", err))),
                            }
                            }
                            _ => Err(ClientError::AlreadyExistResponseAndFailToFoundIt(format!(
                                "Response error code was {}",
                                error_code
                            ))),
                        }
                    }
                    Err(err) => Err(ClientError::AlreadyExistResponseAndFailToFoundIt(
                        "Fail when trying to determine response error code".to_string(),
                    )),
                }
            }
            _ => Err(ClientError::UnexpectedResponse(format!(
                "Response status was {}",
                &response.status()
            ))),
        }
    }

    fn get_error_code(&self, response: &Response) -> Result<u16, ClientError> {
        match response.json::<Value>()?["code"].as_u64() {
            Some(code) => Ok(code as u16),
            None => Err(ClientError::DecodingResponseError(format!(
                "Fail to find and convert error code"
            ))),
        }
    }

    fn find_existing(
        &self,
        absolute_file_path: String,
        parent_id: Option<ContentId>,
    ) -> Result<ContentId, ClientError> {
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
                return Ok(remote_content.content_id);
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
    ) {
        // TODO : currently manage only files

        let mut form = multipart::Form::new()
            .file("files", absolute_file_path)
            .unwrap();

        // TODO : need to check if response is 200 !!
        self.client
            .request(
                Method::PUT,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/files/{}/raw/{}",
                    content_id, file_name,
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .multipart(form)
            .send()
            .unwrap();
    }

    pub fn trash_content(&self, content_id: ContentId) {
        // TODO : need to check if response is 200 !!
        self.client
            .request(
                Method::PUT,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/contents/{}/trashed",
                    content_id,
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()
            .unwrap();
    }

    pub fn get_remote_content(&self, content_id: ContentId) -> RemoteContent {
        // TODO : Manage other than files
        self.client
            .request(
                Method::GET,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/contents/{}",
                    content_id
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()
            .unwrap()
            .json::<RemoteContent>()
            .unwrap()
    }

    pub fn build_relative_path(&self, content: &RemoteContent) -> String {
        if let Some(parent_id) = content.parent_id {
            let mut path_parts: Vec<String> = vec![content.filename.clone()];
            let mut last_seen_parent_id = parent_id;
            loop {
                // TODO : need to check if response is 200 !!
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
                    .send()
                    .unwrap();
                let folder = response.json::<RemoteContent>().unwrap();
                path_parts.push(folder.filename);
                if let Some(folder_parent_id) = folder.parent_id {
                    last_seen_parent_id = folder_parent_id;
                } else {
                    // TODO : this is very ugly code !
                    let mut relative_path_string = "".to_string();
                    for path_part in path_parts.iter().rev() {
                        let relative_path = Path::new(&relative_path_string).join(path_part);
                        relative_path_string = relative_path.to_str().unwrap().to_string();
                    }
                    return relative_path_string;
                }
            }
        } else {
            content.filename.clone()
        }
    }

    pub fn get_file_content_response(&self, content_id: ContentId, file_name: String) -> Response {
        // TODO : need to check if response is 200 !!
        self.client
            .request(
                Method::GET,
                format!(
                    "https://tracim.bux.fr/api/workspaces/4/files/{}/raw/{}",
                    content_id, file_name,
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()
            .unwrap()
    }

    pub fn get_remote_contents(
        &self,
        parent_id: Option<ParentIdParameter>,
    ) -> Result<Vec<RemoteContent>, ClientError> {
        let url = match parent_id {
            Some(parent_id) => format!("?parent_ids={}", parent_id.to_parameter_value()),
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

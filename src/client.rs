use std::path::Path;

use reqwest::blocking::{multipart, Response};
use reqwest::Method;

use serde_json::Value;

use crate::{
    remote::RemoteContent,
    types::{ContentId, ContentType},
};

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
    ) -> ContentId {
        // TODO : currently manage only files

        let mut form = multipart::Form::new();
        if let Some(parent_content_id) = parent_content_id {
            form = form.text("parent_id", parent_content_id.to_string());
        };
        form = form.file("files", absolute_file_path).unwrap();

        // TODO : need to check if response is 200 !!
        let r = self
            .client
            .request(Method::POST, "https://tracim.bux.fr/api/workspaces/4/files")
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .multipart(form)
            .send()
            .unwrap();

        r.json::<Value>().unwrap().as_object().unwrap()["content_id"]
            .as_i64()
            .unwrap() as ContentId
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
}

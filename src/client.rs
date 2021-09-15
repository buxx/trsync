use std::path::Path;

use reqwest::blocking::multipart;
use reqwest::Method;

use serde_json::{Map, Value};

use crate::types::{ContentId, ContentType};

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
            .request(Method::POST, "https://tracim.bux.fr/api/workspaces/3/files")
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
                    "https://tracim.bux.fr/api/workspaces/3/files/{}/raw/{}",
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
                    "https://tracim.bux.fr/api/workspaces/3/contents/{}/trashed",
                    content_id,
                ),
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()
            .unwrap();
    }
}

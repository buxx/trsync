use crate::operation::ContentId;

pub struct Client {
    tracim_api_key: String,
    tracim_user_name: String,
}

impl Client {
    pub fn new(tracim_api_key: String, tracim_user_name: String) -> Self {
        Self {
            tracim_api_key,
            tracim_user_name,
        }
    }

    pub fn post_content(
        &self,
        absolute_file_path: String,
        file_name: String,
        content_type: String,
        parent_content_id: Option<ContentId>,
    ) -> ContentId {
        42
    }
}

use std::fmt::Display;

pub struct UserId(pub i32);

impl Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

pub enum UserRequest {
    OpenMonitorWindow,
    OpenConfigurationWindow,
    Quit,
}

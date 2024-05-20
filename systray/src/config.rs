use std::path::Path;

use ini::Ini;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Config {
    #[cfg(target_os = "linux")]
    pub icons_path: String,
}
impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let user_home_folder_path = match dirs::home_dir() {
            Some(folder) => folder,
            None => return Err(Error::UnableToFindHomeUser),
        };
        let config_file_path = if cfg!(target_os = "windows") {
            user_home_folder_path
                .join("AppData")
                .join("Local")
                .join("trsync.conf")
        } else {
            user_home_folder_path.join(".trsync.conf")
        };

        let config_ini = match Ini::load_from_file(&config_file_path) {
            Ok(content) => content,
            Err(error) => {
                return Err(Error::ReadConfigError(format!(
                    "Unable to read or load '{:?}' config file : '{}'",
                    &config_file_path, error,
                )))
            }
        };
        Self::from_ini(config_ini)
    }

    #[cfg(target_os = "linux")]
    pub fn from_ini(config_ini: Ini) -> Result<Self, Error> {
        let icons_path = match config_ini.get_from(Some("server"), "icons_path") {
            Some(icon_path_) => Path::new(icon_path_),
            None => {
                return Err(Error::ReadConfigError(
                    "Unable to find server icons_path config".to_string(),
                ))
            }
        };
        Ok(Self {
            icons_path: icons_path.to_str().unwrap().to_string(),
        })
    }

    #[cfg(target_os = "windows")]
    pub fn from_ini(_config_ini: Ini) -> Result<Self, Error> {
        Ok(Self {})
    }
}

use ini::Ini;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub trsync_manager_bin_path: String,
    pub trsync_manager_configure_bin_path: String,
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
                    "Unable to read or load '{:?}' config file : {}",
                    &config_file_path, error,
                )))
            }
        };
        Self::from_ini(config_ini)
    }

    pub fn from_ini(config_ini: Ini) -> Result<Self, Error> {
        let trsync_manager_bin_path = match config_ini
            .get_from(Some("server"), "trsync_manager_bin_path")
        {
            Some(value) => value,
            None => {
                return Err(Error::ReadConfigError(
                    "Unable to read trsync_manager_bin_path config from server section".to_string(),
                ))
            }
        }
        .to_string();

        let trsync_manager_configure_bin_path =
            match config_ini.get_from(Some("server"), "trsync_manager_configure_bin_path") {
                Some(value) => value,
                None => return Err(Error::ReadConfigError(
                    "Unable to read trsync_manager_configure_bin_path config from server section"
                        .to_string(),
                )),
            }
            .to_string();

        Ok(Self {
            trsync_manager_bin_path,
            trsync_manager_configure_bin_path,
        })
    }
}

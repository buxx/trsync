use std::time::Duration;

use ini::Ini;

use crate::{error::Error, model::Instance, security};

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_timeout: Duration,
    pub local_folder: Option<String>,
    pub instances: Vec<Instance>,
    pub allow_raw_passwords: bool,
}
impl Config {
    pub fn from_env(allow_raw_passwords: bool) -> Result<Self, Error> {
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
        Self::from_ini(config_ini, allow_raw_passwords)
    }

    pub fn from_ini(config_ini: Ini, allow_raw_passwords: bool) -> Result<Self, Error> {
        let os_username = whoami::username();
        let listen_timeout = match config_ini
            .get_from(Some("server"), "listen_timeout")
            .unwrap_or("60")
            .parse::<u64>()
        {
            Ok(timeout_seconds) => Duration::from_secs(timeout_seconds),
            Err(_) => {
                return Err(Error::ReadConfigError(
                    "Unable to read listen_timeout config from server section".to_string(),
                ))
            }
        };
        let local_folder = match config_ini.get_from(Some("server"), "local_folder") {
            Some(value) => {
                if value.trim() == "" {
                    None
                } else {
                    Some(value.to_string())
                }
            }
            None => {
                return Err(Error::ReadConfigError(
                    "Unable to read local_folder config from server section".to_string(),
                ))
            }
        };

        let mut instances = vec![];
        let instances_raw = match config_ini.get_from(Some("server"), "instances") {
            Some(value) => value,
            None => {
                return Err(Error::ReadConfigError(
                    "Unable to read instances config from server section".to_string(),
                ))
            }
        }
        .to_string();
        for instance_name in instances_raw
            .split(",")
            .map(|v| v.trim().to_string())
            .collect::<Vec<String>>()
        {
            if instance_name.trim().is_empty() {
                continue;
            }
            let section_name = format!("instance.{}", instance_name);
            let address = match config_ini.get_from(Some(&section_name), "address") {
                Some(value) => value,
                None => {
                    return Err(Error::ReadConfigError(format!(
                        "Unable to read address config from {} section",
                        &section_name
                    )))
                }
            }
            .to_string();
            let username = match config_ini.get_from(Some(&section_name), "username") {
                Some(value) => value,
                None => {
                    return Err(Error::ReadConfigError(format!(
                        "Unable to read username config from {} section",
                        &section_name
                    )))
                }
            }
            .to_string();
            let unsecure = match config_ini.get_from(Some(&section_name), "username") {
                Some(value) => vec!["true", "True", "t", "T", "1"].contains(&value),
                None => false,
            };
            let mut workspaces_ids = vec![];
            let workspace_ids_raw = match config_ini.get_from(Some(&section_name), "workspaces_ids")
            {
                Some(value) => value,
                None => {
                    return Err(Error::ReadConfigError(format!(
                        "Unable to read workspaces_ids config from {} section",
                        &section_name
                    )))
                }
            };

            // try to get password from keyring
            let password = match security::get_password(&address, &os_username) {
                Ok(password_) => password_,
                Err(error) => {
                    if !allow_raw_passwords {
                        log::error!(
                            "Unable to read password from keyring for instance '{}' and user '{os_username}', this instance will be ignored : '{}'",
                            &address,
                            error,
                        );
                        continue;
                    }

                    match config_ini.get_from(Some(&section_name), "password") {
                        Some(password) => password.to_string(),
                        None => {
                            log::error!(
                                "Unable to read password from config for instance '{}' and user '{os_username}', this instance will be ignored : '{}'",
                                &address,
                                error,
                            );
                            continue;
                        }
                    }
                }
            };

            if workspace_ids_raw.trim() != "" {
                for workspace_id_raw in workspace_ids_raw
                    .split(",")
                    .map(|v| v.trim().to_string())
                    .collect::<Vec<String>>()
                {
                    match workspace_id_raw.parse::<u32>() {
                    Ok(workspace_id) => workspaces_ids.push(workspace_id),
                    Err(_) => {
                        return Err(Error::ReadConfigError(format!(
                            "Unable to understand workspace_id from workspace_ids config from {} section",
                            &section_name
                        )))
                    }
                };
                }
            }

            instances.push(Instance {
                name: instance_name,
                address,
                unsecure,
                username,
                password,
                workspaces_ids,
            })
        }

        Ok(Self {
            listen_timeout,
            local_folder,
            instances,
            allow_raw_passwords,
        })
    }
}

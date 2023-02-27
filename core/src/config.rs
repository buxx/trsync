use std::time::Duration;

use anyhow::{Context, Error, Result};

use ini::Ini;

use crate::{
    instance::{Instance, InstanceId, WorkspaceId},
    security::get_password,
    utils::strbool,
};

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub listen_timeout: Duration,
    pub local_folder: Option<String>,
    pub instances: Vec<Instance>,
    pub allow_raw_passwords: bool,
    pub prevent_delete_sync: bool,
}
impl ManagerConfig {
    pub fn from_env(allow_raw_passwords: bool) -> Result<Self> {
        let user_home_folder_path = dirs::home_dir().context("Unable to determine home folder")?;
        let config_file_path = if cfg!(target_os = "windows") {
            user_home_folder_path
                .join("AppData")
                .join("Local")
                .join("trsync.conf")
        } else {
            user_home_folder_path.join(".trsync.conf")
        };

        let config_ini = Ini::load_from_file(&config_file_path).context(format!(
            "Error when loading config file at '{}'",
            config_file_path.display()
        ))?;
        Self::from_ini(config_ini, allow_raw_passwords)
    }

    pub fn from_ini(config_ini: Ini, allow_raw_passwords: bool) -> Result<Self> {
        let os_username = whoami::username();
        let server = config_ini
            .section(Some("server"))
            .context("Missing \"server\" section in config")?;

        let listen_timeout = Duration::from_secs(
            server
                .get("listen_timeout")
                .unwrap_or("60")
                .parse::<u64>()
                .context("Unable to read listen_timeout config from server section")?,
        );
        let prevent_delete_sync = strbool(server.get("prevent_delete_sync").unwrap_or("1"));
        let local_folder = server.get("local_folder").and_then(|v| Some(v.to_string()));
        let instances_ids: Vec<InstanceId> = server
            .get("instances")
            .unwrap_or("")
            .split(",")
            .filter(|v| !v.trim().is_empty())
            .map(|v| InstanceId(v.to_string()))
            .collect();

        let mut instances = vec![];
        for instance_id in instances_ids {
            let section_name = format!("instance.{}", instance_id);
            let instance_config = config_ini
                .section(Some(section_name.clone()))
                .context(format!("Missing '{}' section in config", section_name))?;
            let address = instance_config
                .get("address")
                .context(format!(
                    "Unable to read address config from {} section",
                    &section_name
                ))?
                .to_string();
            let username = instance_config
                .get("username")
                .context(format!(
                    "Unable to read username config from '{}' section",
                    &section_name
                ))?
                .to_string();
            let unsecure = strbool(instance_config.get("unsecure").unwrap_or("0"));
            let (workspaces_ids, errors): (Vec<_>, Vec<_>) = server
                .get("workspaces_ids")
                .unwrap_or("")
                .split(",")
                .into_iter()
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.parse::<i32>())
                .partition(Result::is_ok);
            if errors.len() > 0 {
                return Result::Err(Error::msg(format!(
                    "Some workspaces ids are invalid in '{}' section",
                    &section_name
                )));
            }
            let workspaces_ids = workspaces_ids
                .into_iter()
                .filter_map(|v| match v {
                    Ok(v) => Some(v),
                    Err(_) => None,
                })
                .map(|v| WorkspaceId(v))
                .collect();

            // try to get password from keyring
            let password = match get_password(&address, &os_username) {
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

            instances.push(Instance {
                name: instance_id,
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
            prevent_delete_sync,
        })
    }
}

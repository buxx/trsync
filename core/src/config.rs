use std::path::PathBuf;

use anyhow::{Context, Error, Result};

use ini::Ini;

use crate::{
    instance::{Instance, InstanceId, WorkspaceId},
    security::get_password,
    utils::strbool,
};

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub local_folder: String,
    pub icons_path: Option<String>,
    pub instances: Vec<Instance>,
    pub allow_raw_passwords: bool,
    pub prevent_delete_sync: bool,
}
impl ManagerConfig {
    fn path() -> Result<PathBuf> {
        let user_home_folder_path = dirs::home_dir().context("Unable to determine home folder")?;

        if cfg!(target_os = "windows") {
            Ok(user_home_folder_path
                .join("AppData")
                .join("Local")
                .join("trsync.conf"))
        } else {
            Ok(user_home_folder_path.join(".trsync.conf"))
        }
    }

    pub fn from_env(allow_raw_passwords: bool) -> Result<Self> {
        let config_file_path = Self::path()?;
        let config_ini = Ini::load_from_file(&config_file_path).context(format!(
            "Error when loading config file at '{}'",
            config_file_path.display()
        ))?;
        Self::from_ini(config_ini, allow_raw_passwords)
    }

    pub fn from_ini(config_ini: Ini, allow_raw_passwords: bool) -> Result<Self> {
        let os_username = whoami::username();
        let user_home_folder_path = dirs::home_dir().context("Unable to determine home folder")?;
        let server = config_ini
            .section(Some("server"))
            .context("Missing \"server\" section in config")?;

        let prevent_delete_sync = strbool(server.get("prevent_delete_sync").unwrap_or("1"));
        let local_folder = server
            .get("local_folder")
            .map(|v| v.to_string())
            .unwrap_or_else(|| user_home_folder_path.join("Tracim").display().to_string())
            .to_string();
        let icons_path = server.get("icons_path").map(|v| v.to_string());
        let instances_ids: Vec<InstanceId> = server
            .get("instances")
            .unwrap_or("")
            .split(',')
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
            let (workspaces_ids, errors): (Vec<_>, Vec<_>) = instance_config
                .get("workspaces_ids")
                .unwrap_or("")
                .split(',')
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.parse::<i32>())
                .partition(Result::is_ok);
            if !errors.is_empty() {
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
                .map(WorkspaceId)
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
            local_folder,
            icons_path,
            instances,
            allow_raw_passwords,
            prevent_delete_sync,
        })
    }

    pub fn write(&self) -> Result<()> {
        let config_file_path = Self::path()?;
        let conf: Ini = self.clone().into();
        conf.write_to_file(&config_file_path)
            .context(format!("Write config to '{}'", config_file_path.display()))?;
        Ok(())
    }
}

impl Into<Ini> for ManagerConfig {
    fn into(self) -> Ini {
        let mut conf = Ini::new();

        let instances_ids = self
            .instances
            .iter()
            .map(|i| i.name.to_string())
            .collect::<Vec<String>>()
            .join(",");
        let local_folder = self.local_folder.clone();
        let prevent_delete_sync = self.prevent_delete_sync.to_string();

        conf.with_section(Some("server"))
            .set("instances", instances_ids)
            .set("local_folder", local_folder)
            .set("prevent_delete_sync", prevent_delete_sync);

        if let Some(icons_path) = self.icons_path {
            conf.with_section(Some("server"))
                .set("icons_path", icons_path);
        }

        for instance in &self.instances {
            let address = instance.address.clone();
            let username = instance.username.clone();
            let unsecure = instance.unsecure.to_string();
            let workspaces_ids = instance
                .workspaces_ids
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<String>>()
                .join(",");

            conf.with_section(Some(format!("instance.{}", instance.name)))
                .set("address", address)
                .set("username", username)
                .set("unsecure", unsecure)
                .set("workspaces_ids", workspaces_ids);
        }

        conf
    }
}

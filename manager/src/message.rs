use crate::config::Config;

#[derive(Debug)]
pub enum DaemonControlMessage {
    Reload(Config),
    Stop,
    StorePassword(String, String), // instance_name, password
}

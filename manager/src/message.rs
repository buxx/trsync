use crate::config::Config;

#[derive(Debug)]
pub enum DaemonControlMessage {
    Reload(Config),
    Stop,
}

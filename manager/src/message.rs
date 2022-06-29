use crate::config::Config;

#[derive(Debug)]
pub enum DaemonMessage {
    Reload(Config),
    Stop,
}

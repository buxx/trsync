use crate::config::Config;

pub enum Message {}

#[derive(Debug)]
pub enum DaemonMessage {
    ReloadFromConfig(Config),
    Stop,
}

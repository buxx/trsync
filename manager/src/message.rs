use trsync_core::config::ManagerConfig;

#[derive(Debug)]
pub enum DaemonMessage {
    Reload(ManagerConfig),
    Stop,
}

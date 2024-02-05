use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unexpected error : '{0}'")]
    Unexpected(String),
    #[error("Unable to determine use home path")]
    UnableToFindHomeUser,
    #[error("Read config error : '{0}'")]
    ReadConfigError(String),
    #[error("Manager error error : '{0}'")]
    ManagerError(trsync_manager::error::Error),
}

impl From<trsync_manager::error::Error> for Error {
    fn from(error: trsync_manager::error::Error) -> Self {
        Error::ManagerError(error)
    }
}

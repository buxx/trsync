#[derive(Debug)]
pub enum Error {
    UnexpectedError(String),
    UnableToFindHomeUser,
    ReadConfigError(String),
    ManagerError(trsync_manager::error::Error),
}

impl From<trsync_manager::error::Error> for Error {
    fn from(error: trsync_manager::error::Error) -> Self {
        Error::ManagerError(error)
    }
}

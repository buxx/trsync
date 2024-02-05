use std::{io, str::Utf8Error};
use strum_macros::Display;
use thiserror::Error;

#[derive(Debug, Error, Display)]
pub enum Error {
    UnIndexedRelativePath(String),
    UnexpectedError(String),
    PathCastingError(String),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::UnexpectedError(format!("{:?}", error))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::UnexpectedError(format!("{:?}", error))
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(error: std::time::SystemTimeError) -> Self {
        Error::UnexpectedError(format!("{:?}", error))
    }
}

impl From<std::path::StripPrefixError> for Error {
    fn from(error: std::path::StripPrefixError) -> Self {
        Error::UnexpectedError(format!("Unable to strip prefix {:?}", error))
    }
}

impl From<notify::Error> for Error {
    fn from(error: notify::Error) -> Self {
        Error::UnexpectedError(format!("Notify error {:?}", error))
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Error::UnexpectedError(format!("utf8 error {:?}", error))
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::UnexpectedError(format!("reqwest error {:?}", error))
    }
}

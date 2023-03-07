use std::{fmt::Display, io};

use crossbeam_channel::RecvError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    ChannelError(RecvError),
    UnexpectedError(String),
    UnavailableNetwork(String),
}

impl From<RecvError> for Error {
    fn from(error: RecvError) -> Self {
        Self::ChannelError(error)
    }
}

#[derive(Debug)]
pub enum ClientError {
    RequestError(String),
    Unauthorized,
    UnexpectedResponse(String),
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::UnexpectedError(format!("Error happen when make request : {:?}", error))
    }
}

impl From<notify::Error> for Error {
    fn from(error: notify::Error) -> Self {
        Error::UnexpectedError(format!("Notify error {:?}", error))
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::UnexpectedError(format!("{:?}", error))
    }
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::RequestError(message) => {
                write!(f, "Error during http request: '{}'", message)
            }
            ClientError::Unauthorized => write!(f, "Error during http request: Unauthorized"),
            ClientError::UnexpectedResponse(message) => {
                write!(f, "Unexpected http response: '{}'", message)
            }
        }
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(error: reqwest::Error) -> Self {
        Self::RequestError(format!("Error happen when make request : {:?}", error))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ChannelError(message) => {
                write!(f, "Channel communication error : '{}'", message)
            }
            Error::UnexpectedError(message) => write!(f, "Unexpected error : '{}'", message),
            Error::UnavailableNetwork(message) => write!(f, "Network error : '{}'", message),
        }
    }
}

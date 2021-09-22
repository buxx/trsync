use std::{fmt, io};

use crate::types::{AbsoluteFilePath, ContentId, RevisionId};

#[derive(Debug, Clone)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self {
            message: format!("io error: {}", err),
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    InputFileError(AbsoluteFilePath),
    RequestError(String),
    UnexpectedResponse(String),
    AlreadyExistResponse(ContentId, RevisionId),
    AlreadyExistResponseAndFailToFoundIt(String),
    NotFoundResponse(String),
    DecodingResponseError(String),
}

impl From<reqwest::Error> for ClientError {
    fn from(error: reqwest::Error) -> Self {
        Self::RequestError(format!("Error happen when make request : {:?}", error))
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ClientError::InputFileError(absolute_file_path) => {
                format!("Error when reading input file '{}'", absolute_file_path)
            }
            ClientError::RequestError(message) => {
                format!("Error when making request : {}", message)
            }
            ClientError::UnexpectedResponse(message) => {
                format!("UnExpected response : {}", message)
            }
            ClientError::AlreadyExistResponse(content_id, revision_id) => {
                format!("Content already exist : {}({})", content_id, revision_id)
            }
            ClientError::AlreadyExistResponseAndFailToFoundIt(message) => format!(
                "Already exist but fail to found remote content : {}",
                message
            ),
            ClientError::NotFoundResponse(message) => format!("Not found : {}", message),
            ClientError::DecodingResponseError(message) => format!("Decoding error : {}", message),
        };
        write!(f, "{}", message)
    }
}

#[derive(Debug)]
pub enum OperationError {
    FailToCreateContentOnRemote(String),
    FailToCreateContentOnLocal(String),
    UnexpectedError(String),
}

impl From<ClientError> for OperationError {
    fn from(err: ClientError) -> Self {
        OperationError::UnexpectedError(format!("{:?}", err))
    }
}

impl From<io::Error> for OperationError {
    fn from(error: io::Error) -> Self {
        OperationError::UnexpectedError(format!("{:?}", error))
    }
}

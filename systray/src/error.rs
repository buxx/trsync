#[derive(Debug)]
pub enum Error {
    UnableToFindHomeUser,
    ReadConfigError(String),
}

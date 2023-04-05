use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error returned by the API: status code {0} on path {1}")]
    ApiError(String, String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("UTF8 error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("UTF8 error: {0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("HTTP error: {0}")]
    HTTPError(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Unknown error")]
    Unknown,
}

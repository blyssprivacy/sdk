use thiserror::Error;

/// Error type for Blyss.
#[derive(Error, Debug)]
pub enum Error {
    /// An error returned by the API.
    #[error("Error returned by the API: status code {0} on path {1}")]
    ApiError(String, String),
    /// An error parsing or processing JSON.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// An error parsing or processing UTF-8.
    #[error("UTF8 error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    /// An error parsing or processing UTF-8.
    #[error("UTF8 error: {0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    /// An error making HTTP requests.
    #[error("HTTP error: {0}")]
    HTTPError(#[from] reqwest::Error),
    /// A wrapped io::Error.
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    /// An error caused by failing to call `setup()` before using `private_read()`.
    #[error("Must call setup() before using private_read()")]
    NeedSetup,
    /// An unknown error.
    #[error("Unknown error")]
    Unknown,
}

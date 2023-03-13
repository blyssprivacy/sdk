use std::{fmt::Display, sync::PoisonError};

use actix_http::body::BoxBody;
use actix_web::{HttpResponse, ResponseError};

#[derive(Debug)]
pub enum Error {
    InvalidLength(usize, usize),
    IoError(std::io::Error),
    NotFound,
    Unknown,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IoError(io_error) => write!(f, "{}", io_error),
            Error::NotFound => write!(f, "not found"),
            Error::Unknown => write!(f, "unknown err"),
            Error::InvalidLength(got, expected) => {
                write!(f, "bad length: got {}, expected {}", got, expected)
            }
        }
    }
}

impl std::error::Error for Error {}

// TODO: add status_code() implementation to give better error info
impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::with_body(self.status_code(), self.to_string()).map_into_boxed_body()
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Error::Unknown
    }
}

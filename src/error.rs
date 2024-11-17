use std::io;
use unrar::error::UnrarError;
use zip::result::{InvalidPassword, ZipError};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Zip(ZipError),
    Unrar(UnrarError),
    InvalidPassword,
    EncodingError,
}

impl From<ZipError> for Error {
    fn from(value: ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<InvalidPassword> for Error {
    fn from(_: InvalidPassword) -> Self {
        Self::InvalidPassword
    }
}

impl From<UnrarError> for Error {
    fn from(e: UnrarError) -> Self {
        Self::Unrar(e)
    }
}

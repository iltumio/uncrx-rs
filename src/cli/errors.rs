use clap::error::ErrorKind;
use std::{error::Error, fmt};

#[derive(Debug, Clone)]
pub enum UncrxCliError {
    UnsupportedFileType,
    NotFound(String),
}

impl Error for UncrxCliError {}

impl fmt::Display for UncrxCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UncrxCliError::UnsupportedFileType => {
                write!(f, "Unsupported file type. Only CRX files are supported")
            }
            UncrxCliError::NotFound(path) => write!(f, "{} not found", path),
        }
    }
}

impl Into<ErrorKind> for UncrxCliError {
    fn into(self) -> ErrorKind {
        match self {
            UncrxCliError::UnsupportedFileType => ErrorKind::InvalidValue,
            UncrxCliError::NotFound(_) => ErrorKind::Io,
        }
    }
}

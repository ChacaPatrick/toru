use std::io;

use crate::naming::NameError;

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    Io(io::Error),
    Image(image::ImageError),
    /// Another `Store`, usually another running instance of the app, has this folder open.
    AlreadyOpen,
    NotFound,
    /// The pixels are empty or differ from the record's size, or the thumbnail width is zero.
    BadDimensions,
    InvalidName(NameError),
    /// Another capture from the same day already has that name.
    NameTaken,
    /// The picture would show what a saved redaction hid, though its original was deleted.
    WouldReveal,
}

impl From<NameError> for StoreError {
    fn from(error: NameError) -> Self {
        StoreError::InvalidName(error)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Database(error)
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        StoreError::Io(error)
    }
}

impl From<image::ImageError> for StoreError {
    fn from(error: image::ImageError) -> Self {
        StoreError::Image(error)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

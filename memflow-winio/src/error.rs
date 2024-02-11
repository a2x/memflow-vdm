use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("Windows error: {0}")]
    Windows(#[from] windows::core::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

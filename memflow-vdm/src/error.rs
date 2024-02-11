use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to map physical memory")]
    MapPhysMem,

    #[error("Failed to unmap physical memory")]
    UnmapPhysMem,

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("Windows error: {0}")]
    Windows(#[from] windows::core::Error),

    #[error("{0}")]
    Other(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("driver not found")]
    DriverNotFound,

    #[error("unable to map physical memory")]
    MapPhysicalMemory,

    #[error("unable to unmap physical memory")]
    UnmapPhysicalMemory,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Windows(#[from] windows::core::Error),

    #[error("{0}")]
    Other(&'static str),
}

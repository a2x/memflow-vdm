use memflow::types::{Address, PhysicalAddress};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to open device \"{device_path}\": {source}")]
    OpenDevice {
        device_path: String,

        #[source]
        source: windows::core::Error,
    },

    #[error("unable to map physical memory at {addr:#X}")]
    MapPhysicalMemory { addr: PhysicalAddress },

    #[error("unable to unmap physical memory at {addr:#X}")]
    UnmapPhysicalMemory { addr: Address },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Windows(#[from] windows::core::Error),
}

impl From<Error> for memflow::error::Error {
    fn from(err: Error) -> Self {
        use memflow::error::{Error as MemflowError, ErrorKind, ErrorOrigin};

        MemflowError(ErrorOrigin::Connector, ErrorKind::Uninitialized).log_error(err.to_string())
    }
}

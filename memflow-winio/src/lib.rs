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
    MapPhysMem = 0x80102040,
    UnmapPhysMem = 0x80102044,
}

#[derive(Debug)]
#[repr(C)]
struct MapUnmapPhysMemRequest {
    /// Size of the memory to map.
    size: u64,

    /// Physical address to map.
    phys_addr: PhysAddr,

    /// Handle to the section representing the mapped memory region.
    section_handle: HANDLE,

    /// Virtual address of the mapped memory.
    virt_addr: VirtAddr,

    /// Handle to the object representing the mapped memory.
    obj_handle: HANDLE,
}

impl Default for MapUnmapPhysMemRequest {
    #[inline]
    fn default() -> Self {
        Self {
            size: 0,
            phys_addr: PhysAddr::new(0),
            section_handle: HANDLE::default(),
            virt_addr: VirtAddr::new(0),
            obj_handle: HANDLE::default(),
        }
    }
}

impl MapPhysMemResult for MapUnmapPhysMemRequest {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn virt_addr(&self) -> VirtAddr {
        self.virt_addr
    }
}

pub trait MapPhysMemResultExt: MapPhysMemResult {
    /// Returns the handle to the object representing the mapped memory.
    fn obj_handle(&self) -> HANDLE;

    /// Returns the handle to the section representing the mapped memory region.
    fn section_handle(&self) -> HANDLE;
}

impl MapPhysMemResultExt for MapUnmapPhysMemRequest {
    #[inline]
    fn obj_handle(&self) -> HANDLE {
        self.obj_handle
    }

    #[inline]
    fn section_handle(&self) -> HANDLE {
        self.section_handle
    }
}

#[derive(Clone, Debug)]
pub struct WinIo {
    /// Handle to the vulnerable driver.
    pub handle: HANDLE,
}

impl WinIo {
    pub fn new() -> Result<Self> {
        let handle = unsafe {
            CreateFileA(
                s!(r"\\.\WinIo"),
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

impl Drop for WinIo {
    #[inline]
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe { _ = CloseHandle(self.handle) }
        }
    }
}

impl PhysMem for WinIo {
    fn map_phys_mem(
        &self,
        addr: PhysAddr,
        size: usize,
    ) -> memflow_vdm::Result<MapPhysMemResultBoxed> {
        let mut req = MapUnmapPhysMemRequest {
            size: size as u64,
            phys_addr: addr,
            ..Default::default()
        };

        unsafe {
            DeviceIoControl(
                self.handle,
                IoCtlCode::MapPhysMem as u32,
                Some(&req as *const _ as *const _),
                mem::size_of::<MapUnmapPhysMemRequest>() as u32,
                Some(&mut req as *mut _ as *mut _),
                mem::size_of::<MapUnmapPhysMemRequest>() as u32,
                None,
                None,
            )?;
        }

        Ok(Box::new(req))
    }

    fn unmap_phys_mem(&self, result: MapPhysMemResultBoxed) -> memflow_vdm::Result<()> {
        let req = result
            .as_any()
            .downcast_ref::<MapUnmapPhysMemRequest>()
            .unwrap();

        let req = MapUnmapPhysMemRequest {
            section_handle: req.section_handle(),
            virt_addr: req.virt_addr(),
            obj_handle: req.obj_handle(),
            ..Default::default()
        };

        unsafe {
            DeviceIoControl(
                self.handle,
                IoCtlCode::UnmapPhysMem as u32,
                Some(&req as *const _ as *const _),
                mem::size_of::<MapUnmapPhysMemRequest>() as u32,
                None,
                0,
                None,
                None,
            )
            .map_err(Into::into)
        }
    }
}

#[connector(name = "winio")]
pub fn create_connector(_args: &ConnectorArgs) -> memflow::error::Result<VdmCtx> {
    let io = WinIo::new().map_err(|_| memflow::error::Error::from(ErrorOrigin::Connector))?;

    Ok(VdmCtx::new(Box::new(io)))
}

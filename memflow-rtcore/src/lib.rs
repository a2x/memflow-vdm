use std::any::Any;
use std::mem;

use memflow::prelude::v1::*;

use memflow_vdm::{PhysicalMemory, Result, *};

use windows::core::s;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{CreateFileA, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING};
use windows::Win32::System::IO::DeviceIoControl;

#[repr(u32)]
enum IoControlCode {
    MapPhysicalMemory = 0x80002000,
    UnmapPhysicalMemory = 0x80002004,
}

struct MapPhysicalMemoryResponse {
    phys_addr: u64,
    size: usize,
    virt_addr: u64,
}

impl PhysicalMemoryResponse for MapPhysicalMemoryResponse {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn phys_addr(&self) -> u64 {
        self.phys_addr
    }

    #[inline]
    fn size(&self) -> usize {
        self.size
    }

    #[inline]
    fn virt_addr(&self) -> u64 {
        self.virt_addr
    }
}

#[repr(C)]
struct PhysicalMemoryMappingRequest {
    addr: u64,
    size: u32,
}

#[derive(Default)]
#[repr(C)]
struct PhysicalMemoryMappingResponse {
    addr: u64,
}

#[repr(C)]
struct PhysicalMemoryUnmappingRequest {
    addr: u64,
}

#[derive(Clone)]
pub struct RtCore64Driver {
    handle: HANDLE,
}

impl RtCore64Driver {
    fn open() -> Result<Self> {
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

impl Drop for RtCore64Driver {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe { _ = CloseHandle(self.handle) }
        }
    }
}

impl PhysicalMemory for RtCore64Driver {
    fn map_phys_mem(&self, addr: u64, size: usize) -> Result<PhysicalMemoryResponseBoxed> {
        let req = PhysicalMemoryMappingRequest {
            addr,
            size: size as u32,
        };

        let mut res = PhysicalMemoryMappingResponse::default();

        unsafe {
            DeviceIoControl(
                self.handle,
                IoControlCode::MapPhysicalMemory as _,
                Some(&req as *const _ as *const _),
                mem::size_of::<PhysicalMemoryMappingRequest>() as _,
                Some(&mut res as *mut _ as *mut _),
                mem::size_of::<PhysicalMemoryMappingResponse>() as _,
                None,
                None,
            )?;
        }

        Ok(Box::new(MapPhysicalMemoryResponse {
            phys_addr: addr,
            size,
            virt_addr: req.addr,
        }))
    }

    fn unmap_phys_mem(&self, mapping: PhysicalMemoryResponseBoxed) -> Result<()> {
        let res = mapping
            .as_any()
            .downcast_ref::<MapPhysicalMemoryResponse>()
            .unwrap();

        let req = PhysicalMemoryUnmappingRequest {
            addr: res.virt_addr(),
        };

        unsafe {
            DeviceIoControl(
                self.handle,
                IoControlCode::UnmapPhysicalMemory as _,
                Some(&req as *const _ as *const _),
                mem::size_of::<PhysicalMemoryUnmappingRequest>() as _,
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
pub fn create_connector<'a>(_args: &ConnectorArgs) -> memflow::error::Result<VdmConnector<'a>> {
    let driver = RtCore64Driver::open().map_err(|_| {
        Error(ErrorOrigin::Connector, ErrorKind::Uninitialized)
            .log_error("Unable to open a handle to the RtCore64 driver")
    })?;

    init_connector(Box::new(driver)).map_err(|_| {
        Error(ErrorOrigin::Connector, ErrorKind::Uninitialized)
            .log_error("Unable to initialize the VDM connector")
    })
}

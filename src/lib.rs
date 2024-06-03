pub use driver::{load_driver, unload_driver};
pub use error::{Error, Result};
pub use phys_ranges::{get_phys_mem_ranges, PhysicalMemoryRange};

use std::any::Any;
use std::sync::{Arc, Mutex};

use dyn_clone::DynClone;

use memflow::prelude::v1::*;

pub mod driver;
pub mod error;
pub mod phys_ranges;

/// A boxed trait object that implements both [`Send`] and [`PhysicalMemoryResponse`] traits.
pub type PhysicalMemoryResponseBoxed = Box<dyn Send + PhysicalMemoryResponse>;

/// A type alias for [`MappedPhysicalMemory`], which is used as the return type for the
/// `create_connector` method.
pub type VdmConnector<'a> = MappedPhysicalMemory<&'a mut [u8], VdmMapData<'a>>;

pub trait PhysicalMemory: Send + Sync + DynClone {
    fn map_phys_mem(&self, addr: u64, size: usize) -> Result<PhysicalMemoryResponseBoxed>;
    fn unmap_phys_mem(&self, mapping: PhysicalMemoryResponseBoxed) -> Result<()>;
}

dyn_clone::clone_trait_object!(PhysicalMemory);

pub trait PhysicalMemoryResponse {
    /// Returns a reference to the underlying data as any type.
    fn as_any(&self) -> &dyn Any;

    /// Returns the target physical address.
    fn phys_addr(&self) -> u64;

    /// Returns the number of bytes mapped.
    fn size(&self) -> usize;

    /// Returns the virtual address of where the physical memory was mapped to.
    fn virt_addr(&self) -> u64;
}

struct PhysicalMemoryRegionMapper {
    mem: Box<dyn PhysicalMemory>,
    mappings: Vec<PhysicalMemoryResponseBoxed>,
}

impl PhysicalMemoryRegionMapper {
    fn new(mem: Box<dyn PhysicalMemory>) -> Result<Self> {
        let ranges = get_phys_mem_ranges()?;

        // Map all physical memory regions.
        let mappings = ranges
            .iter()
            .map(|range| mem.map_phys_mem(range.start_addr, range.size))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { mem, mappings })
    }
}

impl Drop for PhysicalMemoryRegionMapper {
    fn drop(&mut self) {
        for mapping in self.mappings.drain(..) {
            let _ = self.mem.unmap_phys_mem(mapping);
        }
    }
}

pub struct VdmMapData<'a> {
    phys_mapper: Arc<Mutex<PhysicalMemoryRegionMapper>>,
    mem_map: MemoryMap<&'a mut [u8]>,
    addr_map: MemoryMap<(Address, umem)>,
}

impl<'a> VdmMapData<'a> {
    unsafe fn from_addr_map(
        phys_mapper: Arc<Mutex<PhysicalMemoryRegionMapper>>,
        addr_map: MemoryMap<(Address, umem)>,
    ) -> Self {
        Self {
            phys_mapper,
            mem_map: addr_map.clone().into_bufmap_mut(),
            addr_map,
        }
    }
}

impl<'a> AsRef<MemoryMap<&'a mut [u8]>> for VdmMapData<'a> {
    fn as_ref(&self) -> &MemoryMap<&'a mut [u8]> {
        &self.mem_map
    }
}

impl<'a> Clone for VdmMapData<'a> {
    fn clone(&self) -> Self {
        unsafe { Self::from_addr_map(self.phys_mapper.clone(), self.addr_map.clone()) }
    }
}

pub fn init_connector<'a>(mem: Box<dyn PhysicalMemory>) -> Result<VdmConnector<'a>> {
    let phys_mapper = Arc::new(Mutex::new(PhysicalMemoryRegionMapper::new(mem)?));

    let mut addr_map = MemoryMap::new();

    for mapping in &phys_mapper.lock().unwrap().mappings {
        addr_map.push_remap(
            mapping.phys_addr().into(),
            mapping.size() as _,
            mapping.virt_addr().into(),
        );
    }

    let map_data = unsafe { VdmMapData::from_addr_map(phys_mapper, addr_map) };

    Ok(MappedPhysicalMemory::with_info(map_data))
}

pub use error::{Error, Result};

use std::any::Any;
use std::sync::{Arc, Mutex};

use dyn_clone::DynClone;

use memflow::prelude::v1::*;

use phys_ranges::PhysicalMemoryRange;

pub mod error;
pub mod phys_ranges;

pub type PhysicalMemoryResponseBoxed = Box<dyn Send + PhysicalMemoryResponse>;
pub type VdmConnector<'a> = MappedPhysicalMemory<&'a mut [u8], VdmMapData<&'a mut [u8]>>;

pub trait PhysicalMemory: Send + Sync + DynClone {
    fn map_phys_mem(&self, addr: u64, size: usize) -> Result<PhysicalMemoryResponseBoxed>;
    fn unmap_phys_mem(&self, mapping: PhysicalMemoryResponseBoxed) -> Result<()>;
}

dyn_clone::clone_trait_object!(PhysicalMemory);

pub trait PhysicalMemoryResponse {
    fn as_any(&self) -> &dyn Any;
    fn phys_addr(&self) -> u64;
    fn size(&self) -> usize;
    fn virt_addr(&self) -> u64;
}

struct PhysicalMemoryRegionMapper {
    mem: Box<dyn PhysicalMemory>,
    mappings: Vec<PhysicalMemoryResponseBoxed>,
}

impl PhysicalMemoryRegionMapper {
    fn new(mem: Box<dyn PhysicalMemory>, ranges: &[PhysicalMemoryRange]) -> Result<Self> {
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

pub struct VdmMapData<T> {
    mapper: Arc<Mutex<PhysicalMemoryRegionMapper>>,
    mappings: MemoryMap<T>,
    addr_mappings: MemoryMap<(Address, umem)>,
}

impl<T> AsRef<MemoryMap<T>> for VdmMapData<T> {
    #[inline]
    fn as_ref(&self) -> &MemoryMap<T> {
        &self.mappings
    }
}

impl<'a> Clone for VdmMapData<&'a mut [u8]> {
    #[inline]
    fn clone(&self) -> Self {
        unsafe { Self::from_addrmap_mut(self.mapper.clone(), self.addr_mappings.clone()) }
    }
}

impl<'a> VdmMapData<&'a mut [u8]> {
    unsafe fn from_addrmap_mut(
        mapper: Arc<Mutex<PhysicalMemoryRegionMapper>>,
        map: MemoryMap<(Address, umem)>,
    ) -> Self {
        Self {
            mapper,
            mappings: map.clone().into_bufmap_mut(),
            addr_mappings: map,
        }
    }
}

pub fn init_connector<'a>(mem: Box<dyn PhysicalMemory>) -> Result<VdmConnector<'a>> {
    let ranges = phys_ranges::get_phys_mem_ranges()?;

    let mapper = Arc::new(Mutex::new(PhysicalMemoryRegionMapper::new(mem, &ranges)?));

    let mut mem_map = MemoryMap::new();

    for mapping in &mapper.lock().unwrap().mappings {
        mem_map.push_remap(
            mapping.phys_addr().into(),
            mapping.size() as _,
            mapping.virt_addr().into(),
        );
    }

    let map_data = unsafe { VdmMapData::from_addrmap_mut(mapper, mem_map) };

    let mem = MappedPhysicalMemory::with_info(map_data);

    Ok(mem)
}

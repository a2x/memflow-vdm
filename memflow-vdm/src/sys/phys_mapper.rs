use memflow::prelude::*;

use crate::sys::{PhysicalMemoryRange, phys_ranges};
use crate::{PhysicalMemory, PhysicalMemoryMapping, Result};

/// Manages the mapping and unmapping of physical memory.
///
/// This works with any memory provider that implements the [`PhysicalMemory`] trait,
/// and ensures that all physical memory mappings are automatically unmapped when the
/// [`PhysicalMemoryMapper`] is dropped.
pub struct PhysicalMemoryMapper<M: PhysicalMemory> {
    mem: M,
    mappings: Vec<M::Response>,
}

impl<M: PhysicalMemory> PhysicalMemoryMapper<M> {
    /// Creates a new [`PhysicalMemoryMapper`] with the given physical memory provider.
    pub fn with_mem(mem: M) -> Result<Self> {
        Ok(Self {
            mem,
            mappings: Vec::new(),
        })
    }

    /// Maps all physical memory ranges retrieved from the system.
    pub fn map_system_ranges(&mut self) -> Result<()> {
        let ranges = phys_ranges::get_phys_mem_ranges()?;

        self.map_ranges(&ranges)
    }

    /// Maps the given physical memory ranges.
    pub fn map_ranges(&mut self, ranges: &[PhysicalMemoryRange]) -> Result<()> {
        self.mappings = ranges
            .iter()
            .map(|r| self.mem.map_physical_memory(r.addr, r.size))
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    /// Constructs an address map from the physical memory mappings.
    pub fn addr_map(&self) -> MemoryMap<(Address, umem)> {
        self.mappings.iter().fold(MemoryMap::new(), |mut mm, m| {
            mm.push_remap(m.phys_addr().into(), m.size() as _, m.virt_addr().into());

            mm
        })
    }
}

/// Ensures that all physical memory mappings are automatically unmapped when the
/// [`PhysicalMemoryMapper`] is dropped.
///
/// This typically happens when the [`VdmContext`] that references this goes out of scope. E.g.,
/// when the connector has been unloaded by memflow.
impl<M: PhysicalMemory> Drop for PhysicalMemoryMapper<M> {
    fn drop(&mut self) {
        for mapping in &self.mappings {
            let _ = self.mem.unmap_physical_memory(mapping);
        }
    }
}

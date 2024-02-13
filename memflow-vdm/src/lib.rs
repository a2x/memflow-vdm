pub use x86_64::addr::*;

pub use error::{Error, Result};

use std::any::Any;
use std::ptr;

use memflow::cglue;
use memflow::prelude::v1::*;

pub mod error;

/// Type alias for a boxed trait object that implements [`MapPhysMemResult`] and can be sent between
/// threads.
pub type MapPhysMemResultBoxed = Box<dyn MapPhysMemResult + Send>;

/// Size of a 4 KiB page in bytes.
const PAGE_SIZE: u32 = 4096;

/// This trait is used to extend the return value of the [map_phys_mem](PhysMem::map_phys_mem)
/// function.
///
/// Instead of being limited to returning a [`VirtAddr`], we can now return any type that implements
/// the [`MapPhysMemResult`] trait. This is achieved by casting the return value to a `dyn Any`
/// trait object, which can hold any type.
pub trait MapPhysMemResult: Any + Send {
    /// Casts the result to a `dyn Any` trait object.
    fn as_any(&self) -> &dyn Any;

    /// Returns the virtual address of the mapped memory.
    fn virt_addr(&self) -> VirtAddr;
}

impl MapPhysMemResult for VirtAddr {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn virt_addr(&self) -> VirtAddr {
        *self
    }
}

pub trait PhysMem: Send {
    /// Maps physical memory to virtual memory.
    ///
    /// # Arguments
    ///
    /// * `addr` - The physical address to map.
    /// * `size` - The size of the memory to map.
    ///
    /// # Returns
    ///
    /// The virtual address of the mapped memory.
    fn map_phys_mem(&self, addr: PhysAddr, size: usize) -> Result<MapPhysMemResultBoxed>;

    /// Unmaps the previously mapped physical memory at the given virtual address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The virtual address of the memory to unmap.
    fn unmap_phys_mem(&self, result: MapPhysMemResultBoxed) -> Result<()>;

    /// Reads raw bytes from the specified physical address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The physical address to read from.
    /// * `size` - The number of bytes to read.
    ///
    /// # Returns
    ///
    /// A vector containing the raw bytes read from the physical address.
    fn phys_read_raw(&self, addr: PhysAddr, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size];

        self.phys_read_raw_into(addr, &mut buf)?;

        Ok(buf)
    }

    /// Reads raw bytes from the specified physical address into a buffer.
    ///
    /// # Arguments
    ///
    /// * `addr` - The physical address to read from.
    /// * `buf` - The buffer to store the read bytes.
    fn phys_read_raw_into(&self, addr: PhysAddr, buf: &mut [u8]) -> Result<()> {
        let result = self.map_phys_mem(addr, buf.len())?;

        unsafe {
            ptr::copy_nonoverlapping(result.virt_addr().as_ptr(), buf.as_mut_ptr(), buf.len());
        }

        self.unmap_phys_mem(result)
    }

    /// Reads raw bytes from the specified physical address into a buffer in chunks.
    ///
    /// # Arguments
    ///
    /// * `addr` - The physical address to read from.
    /// * `buf` - The buffer to store the read bytes.
    /// * `chunk_size` - The size of each chunk to read.
    fn phys_read_raw_chunked_into(
        &self,
        addr: PhysAddr,
        buf: &mut [u8],
        chunk_size: usize,
    ) -> Result<()> {
        if buf.len() < chunk_size {
            return Err(Error::Other("Buffer too small for chunked read"));
        }

        for (i, chunk) in buf.chunks_mut(chunk_size).enumerate() {
            self.phys_read_raw_into(addr + (i * chunk.len()) as u64, chunk)?;
        }

        Ok(())
    }

    /// Writes raw bytes to the specified physical address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The physical address to write to.
    /// * `buf` - The buffer containing the bytes to write.
    fn phys_write_raw(&self, addr: PhysAddr, buf: &[u8]) -> Result<()> {
        let result = self.map_phys_mem(addr, buf.len())?;

        unsafe {
            ptr::copy_nonoverlapping(buf.as_ptr(), result.virt_addr().as_mut_ptr(), buf.len());
        }

        self.unmap_phys_mem(result)
    }
}

/// A trait for cloning a [`PhysMem`] object.
pub trait PhysMemClone: PhysMem {
    /// Clones the [`PhysMem`] object and returns it as a boxed trait object.
    fn clone_box(&self) -> Box<dyn PhysMemClone>;
}

impl<T> PhysMemClone for T
where
    T: 'static + PhysMem + Clone,
{
    /// Clones the [`PhysMemClone`] trait object into a boxed trait object.
    ///
    /// # Returns
    ///
    /// A boxed trait object implementing the [`PhysMemClone`] trait.
    #[inline]
    fn clone_box(&self) -> Box<dyn PhysMemClone> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn PhysMemClone> {
    #[inline]
    fn clone(&self) -> Box<dyn PhysMemClone> {
        self.clone_box()
    }
}

#[derive(Clone)]
pub struct VdmConnector {
    /// The physical memory object.
    pub mem: Box<dyn PhysMemClone>,

    metadata: Option<PhysicalMemoryMetadata>,
}

impl VdmConnector {
    /// Creates a new `VdmConnector` with the specified physical memory object.
    pub fn new(mem: Box<dyn PhysMemClone>) -> Self {
        Self {
            mem,
            metadata: None,
        }
    }

    /// Creates a new `VdmConnector` with the specified physical memory object and metadata.
    pub fn new_with_metadata(mem: Box<dyn PhysMemClone>, metadata: PhysicalMemoryMetadata) -> Self {
        Self {
            mem,
            metadata: Some(metadata),
        }
    }
}

impl PhysicalMemory for VdmConnector {
    fn phys_read_raw_iter(
        &mut self,
        MemOps {
            inp,
            mut out,
            mut out_fail,
            ..
        }: PhysicalReadMemOps,
    ) -> memflow::error::Result<()> {
        inp.for_each(|CTup3(addr, meta_addr, mut data)| {
            let addr = PhysAddr::new(addr.to_umem());

            // Read in page-sized chunks if the data is larger than a page.
            let result = if data.len() >= PAGE_SIZE as usize {
                self.mem
                    .phys_read_raw_chunked_into(addr, &mut data, PAGE_SIZE as usize)
            } else {
                self.mem.phys_read_raw_into(addr, &mut data)
            };

            match result {
                Ok(_) => {
                    opt_call(out.as_deref_mut(), CTup2(meta_addr, data));
                }
                Err(_) => {
                    opt_call(out_fail.as_deref_mut(), CTup2(meta_addr, data));
                }
            }
        });

        Ok(())
    }

    fn phys_write_raw_iter(
        &mut self,
        MemOps {
            inp,
            mut out,
            mut out_fail,
            ..
        }: PhysicalWriteMemOps,
    ) -> memflow::error::Result<()> {
        inp.for_each(|CTup3(addr, meta_addr, data)| {
            match self
                .mem
                .phys_write_raw(PhysAddr::new(addr.to_umem()), &data)
            {
                Ok(_) => {
                    opt_call(out.as_deref_mut(), CTup2(meta_addr, data));
                }
                Err(_) => {
                    opt_call(out_fail.as_deref_mut(), CTup2(meta_addr, data));
                }
            }
        });

        Ok(())
    }

    fn metadata(&self) -> PhysicalMemoryMetadata {
        self.metadata.unwrap_or(PhysicalMemoryMetadata {
            max_address: Address::from(u64::MAX),
            real_size: u64::MAX,
            readonly: false,
            ideal_batch_size: PAGE_SIZE,
        })
    }
}

cglue_impl_group!(VdmConnector, ConnectorInstance<'a>, {});

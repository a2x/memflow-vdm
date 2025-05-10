pub use memflow_vdm_derive::*;

pub use error::{Error, Result};

use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::Arc;

use memflow::prelude::v1::*;

use sys::{PhysicalMemoryMapper, PhysicalMemoryRange, Service, ServiceManager};

pub mod error;

mod sys;

pub type VdmConnector<'a, M> = MappedPhysicalMemory<&'a mut [u8], VdmContext<'a, M>>;

/// A physical to virtual memory mapping.
pub trait PhysicalMemoryMapping: Send + Sync {
    /// Returns the starting physical address of the memory region that was mapped.
    fn phys_addr(&self) -> PhysicalAddress;

    /// Returns the number of bytes of physical memory that were mapped.
    fn size(&self) -> usize;

    /// Returns the virtual address of where the physical memory has been mapped into the process's
    /// address space.
    fn virt_addr(&self) -> Address;
}

/// An interface for mapping physical memory.
pub trait PhysicalMemory: Send + Sync {
    type Response: PhysicalMemoryMapping;

    /// Maps a region of physical memory into the virtual address space.
    fn map_physical_memory(
        &self,
        phys_addr: PhysicalAddress,
        size: usize,
    ) -> Result<Self::Response>;

    /// Unmaps a previously mapped region of physical memory.
    fn unmap_physical_memory(&self, mapping: &Self::Response) -> Result<()>;
}

/// Builder for constructing a [`VdmConnector`].
pub struct VdmConnectorBuilder<'a, M: PhysicalMemory> {
    mem: Option<M>,
    ranges: Option<Vec<PhysicalMemoryRange>>,
    service: Option<Service>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, M: PhysicalMemory> VdmConnectorBuilder<'a, M> {
    /// Creates a new [`VdmConnectorBuilder`].
    pub fn new() -> Self {
        Self {
            mem: None,
            ranges: None,
            service: None,
            _phantom: PhantomData,
        }
    }

    /// Sets the physical memory provider.
    ///
    /// This method must be called before [`build`](Self::build).
    pub fn with_memory(mut self, mem: M) -> Self {
        self.mem = Some(mem);

        self
    }

    /// Specifies custom physical memory ranges to map.
    ///
    /// By default, [`build`](Self::build) will automatically retrieve and map all physical memory
    /// ranges present on the system. This method overrides that behavior to map only the specified
    /// `ranges` instead.
    pub fn with_ranges<R>(mut self, ranges: R) -> Self
    where
        R: Into<Vec<PhysicalMemoryRange>>,
    {
        self.ranges = Some(ranges.into());

        self
    }

    /// Configures a Windows service for the vulnerable driver.
    ///
    /// This method will create a new service with `service_name` if `driver_path` is provided and
    /// no service with that name already exists. If `driver_path` is `None`, it is assumed that a
    /// service with the given `service_name` already exists, which will be opened.
    ///
    /// The service is started automatically if not already running before the driver handle is
    /// opened via the `init_driver` callback. The service will be automatically stopped when the
    /// `VdmContext` is dropped.
    ///
    /// This method automatically calls [`with_memory`](Self::with_memory) with the resulting driver
    /// instance that implements the [`PhysicalMemory`] trait.
    pub fn with_service<S, P, F>(
        mut self,
        service_name: S,
        driver_path: Option<P>,
        init_driver: F,
    ) -> Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Path>,
        F: FnOnce() -> Result<M>,
    {
        let sm = ServiceManager::local_computer()?;

        let service = match driver_path {
            Some(path) => sm.create_service(service_name.as_ref(), path.as_ref()),
            None => sm.open_service(service_name.as_ref()),
        }?;

        service.start()?;

        let mem = init_driver()?;

        self.service = Some(service);

        Ok(self.with_memory(mem))
    }

    /// Consumes the builder and returns a [`VdmConnector`].
    ///
    /// This will map all physical memory ranges to create a [`VdmConnector`] for use with memflow.
    /// If [`with_ranges`](Self::with_ranges) has been called, only those ranges will be mapped;
    /// otherwise, all physical memory ranges present on the system will be retrieved and mapped.
    ///
    /// # Panics
    ///
    /// Panics if a physical memory provider has not been set via
    /// [`with_memory`](Self::with_memory).
    pub fn build(self) -> Result<VdmConnector<'a, M>> {
        let mut mapper =
            PhysicalMemoryMapper::with_mem(self.mem.expect("physical memory provider not set"))?;

        match self.ranges.as_ref() {
            Some(ranges) => mapper.map_ranges(ranges),
            None => mapper.map_system_ranges(),
        }?;

        let service = self.service.map(Arc::new);

        let ctx = VdmContext::new(mapper.addr_map(), Arc::new(mapper), service);

        Ok(VdmConnector::with_info(ctx))
    }
}

pub struct VdmContext<'a, M: PhysicalMemory> {
    addr_map: MemoryMap<(Address, umem)>,
    mem_map: MemoryMap<&'a mut [u8]>,
    mapper: ManuallyDrop<Arc<PhysicalMemoryMapper<M>>>,
    service: Option<Arc<Service>>,
}

impl<'a, M: PhysicalMemory> VdmContext<'a, M> {
    fn new(
        addr_map: MemoryMap<(Address, umem)>,
        mapper: Arc<PhysicalMemoryMapper<M>>,
        service: Option<Arc<Service>>,
    ) -> Self {
        let mem_map = unsafe { addr_map.clone().into_bufmap_mut() };

        Self {
            addr_map,
            mem_map,
            mapper: ManuallyDrop::new(mapper),
            service,
        }
    }
}

impl<'a, M: PhysicalMemory> AsRef<MemoryMap<&'a mut [u8]>> for VdmContext<'a, M> {
    #[inline]
    fn as_ref(&self) -> &MemoryMap<&'a mut [u8]> {
        &self.mem_map
    }
}

impl<'a, M: PhysicalMemory> Clone for VdmContext<'a, M> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new(
            self.addr_map.clone(),
            (&*self.mapper).clone(),
            self.service.clone(),
        )
    }
}

/// Automatically stops the Windows service associated with the vulnerable driver when `self` is
/// dropped. This only applies if a service was registered using
/// [`VdmConnectorBuilder::with_service`].
impl<'a, M: PhysicalMemory> Drop for VdmContext<'a, M> {
    fn drop(&mut self) {
        // Ensure that all physical memory mappings are unmapped before the service is stopped.
        unsafe {
            ManuallyDrop::drop(&mut self.mapper);
        }

        if let Some(service) = self.service.take() {
            service.stop().ok();
        }
    }
}

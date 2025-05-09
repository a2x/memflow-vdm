use std::io;

use memflow::types::PhysicalAddress;

use winreg::RegKey;
use winreg::enums::*;

/// A contiguous range of physical memory addresses.
#[derive(Clone, Copy, Debug)]
pub struct PhysicalMemoryRange {
    /// The starting physical address of the memory range.
    pub addr: PhysicalAddress,

    /// The size of the memory range, in bytes.
    pub size: usize,
}

/// Retrieves all physical memory ranges present on the system by querying the Windows registry.
pub fn get_phys_mem_ranges() -> Result<Vec<PhysicalMemoryRange>, io::Error> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let key = hklm.open_subkey(r"HARDWARE\RESOURCEMAP\System Resources\Physical Memory")?;
    let key_val = key.get_raw_value(".Translated")?;

    let resource_list = unsafe { &*(key_val.bytes.as_ptr().cast::<CmResourceList>()) };

    let mut ranges = Vec::new();

    for i in 0..resource_list.count {
        let full_descriptor = unsafe { &*resource_list.list.as_ptr().add(i as usize) };

        for j in 0..full_descriptor.partial_resource_list.count {
            let partial_descriptor = unsafe {
                &*full_descriptor
                    .partial_resource_list
                    .partial_descriptors
                    .as_ptr()
                    .add(j as usize)
            };

            let mut size = partial_descriptor.data.size as usize;

            // https://stackoverflow.com/a/48486485
            match partial_descriptor.r#type {
                CmResourceType::Memory => {}
                CmResourceType::MemoryLarge => {
                    let flags = partial_descriptor.flags;

                    if flags & CmResourceMemory::Large40 as u16 != 0 {
                        size <<= 8;
                    } else if flags & CmResourceMemory::Large48 as u16 != 0 {
                        size <<= 16;
                    } else if flags & CmResourceMemory::Large64 as u16 != 0 {
                        size <<= 32;
                    }
                }
                _ => break,
            }

            ranges.push(PhysicalMemoryRange {
                addr: partial_descriptor.data.start.into(),
                size,
            });
        }
    }

    Ok(ranges)
}

#[repr(u16)]
enum CmResourceMemory {
    ReadWrite = 0x0,
    ReadOnly = 0x1,
    WriteOnly = 0x2,
    Prefetchable = 0x4,
    CombinedWrite = 0x8,
    Cacheable = 0x20,
    Large40 = 0x200,
    Large48 = 0x400,
    Large64 = 0x800,
}

#[repr(u8)]
enum CmResourceType {
    Null = 0,
    Port = 1,
    Interrupt = 2,
    Memory = 3,
    Dma = 4,
    DeviceSpecific = 5,
    BusNumber = 6,
    MemoryLarge = 7,
    ConfigData = 128,
    DevicePrivate = 129,
    PcCardConfig = 130,
    MfCardConfig = 131,
    Connection = 132,
}

#[repr(i32)]
enum InterfaceType {
    Undefined = -1,
    Internal,
    Isa,
    Eisa,
    MicroChannel,
    TurboChannel,
    PciBus,
    VmeBus,
    NuBus,
    PcmciaBus,
    CBus,
    MpiBus,
    MpsaBus,
    ProcessorInternal,
    InternalPowerBus,
    PnpIsaBus,
    PnpBus,
    Vmcs,
    AcpiBus,
    MaximumInterfaceType,
}

#[repr(C)]
struct CmFullResourceDescriptor {
    interface_type: InterfaceType,
    bus_number: u32,
    partial_resource_list: CmPartialResourceList,
}

#[repr(C, packed(4))]
struct CmPartialResourceDescriptor {
    r#type: CmResourceType,
    share_disposition: u8,
    flags: u16,
    data: CmPartialResourceDescriptorMemory,
}

#[repr(C, packed(4))]
struct CmPartialResourceDescriptorMemory {
    start: u64,
    size: u64,
}

#[repr(C)]
struct CmPartialResourceList {
    version: u16,
    revision: u16,
    count: u32,
    partial_descriptors: [CmPartialResourceDescriptor; 1],
}

#[repr(C)]
struct CmResourceList {
    count: u32,
    list: [CmFullResourceDescriptor; 1],
}

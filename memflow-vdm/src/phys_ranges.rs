use winreg::enums::*;
use winreg::RegKey;

use crate::error::Result;

#[repr(u16)]
pub enum CmResourceMemory {
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
pub enum CmResourceType {
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
pub enum InterfaceType {
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
pub struct CmFullResourceDescriptor {
    pub interface_type: InterfaceType,
    pub bus_num: u32,
    pub partial_resource_list: CmPartialResourceList,
}

#[repr(C, packed(4))]
pub struct CmPartialResourceDescriptor {
    pub type_: CmResourceType,
    pub share_disposition: u8,
    pub flags: u16,
    pub data: CmPartialResourceDescriptorMemory,
}

#[repr(C, packed(4))]
pub struct CmPartialResourceDescriptorMemory {
    pub start: u64,
    pub size: u64,
}

#[repr(C)]
pub struct CmPartialResourceList {
    pub version: u16,
    pub revision: u16,
    pub count: u32,
    pub partial_descriptors: [CmPartialResourceDescriptor; 1],
}

#[repr(C)]
pub struct CmResourceList {
    pub count: u32,
    pub list: [CmFullResourceDescriptor; 1],
}

#[derive(Debug)]
pub struct PhysicalMemoryRange {
    pub start_addr: u64,
    pub end_addr: u64,
    pub size: usize,
}

pub fn get_phys_mem_ranges() -> Result<Vec<PhysicalMemoryRange>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let key = hklm.open_subkey(r"HARDWARE\RESOURCEMAP\System Resources\Physical Memory")?;
    let key_value = key.get_raw_value(".Translated")?;

    let resource_list = unsafe { &*(key_value.bytes.as_ptr().cast::<CmResourceList>()) };

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

            let mut size = partial_descriptor.data.size;

            // https://stackoverflow.com/a/48486485
            match partial_descriptor.type_ {
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

            let start_addr = partial_descriptor.data.start;
            let end_addr = start_addr + size;

            ranges.push(PhysicalMemoryRange {
                start_addr,
                end_addr,
                size: size as _,
            });
        }
    }

    Ok(ranges)
}

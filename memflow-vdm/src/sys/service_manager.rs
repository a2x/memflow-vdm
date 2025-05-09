use windows::Win32::System::Services::{
    OpenSCManagerA, OpenServiceA, SC_HANDLE, SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS,
};

use windows::core::{PCSTR, Result};

use super::Service;

/// A manager for registering and querying Windows services.
pub struct ServiceManager {
    handle: SC_HANDLE,
}

impl ServiceManager {
    /// Creates a new [`ServiceManager`] by opening the local computer's service control manager.
    pub fn local_computer() -> Result<Self> {
        let handle = unsafe { OpenSCManagerA(None, None, SC_MANAGER_ALL_ACCESS) }?;

        Ok(ServiceManager { handle })
    }

    /// Opens a service by its name.
    pub fn open_service(&self, name: &str) -> Result<Service> {
        let handle = unsafe {
            OpenServiceA(
                self.handle,
                PCSTR::from_raw(format!("{}\0", name).as_ptr()),
                SERVICE_ALL_ACCESS,
            )
        }?;

        Ok(Service::new(handle))
    }
}

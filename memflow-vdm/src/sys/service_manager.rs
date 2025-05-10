use std::ffi::CString;
use std::path::Path;

use windows::Win32::Foundation::ERROR_SERVICE_EXISTS;

use windows::Win32::System::Services::{
    CreateServiceA, OpenSCManagerA, OpenServiceA, SC_HANDLE, SC_MANAGER_ALL_ACCESS,
    SERVICE_ALL_ACCESS, SERVICE_DEMAND_START, SERVICE_ERROR_IGNORE, SERVICE_KERNEL_DRIVER,
};

use windows::core::PCSTR;

use super::Service;

use crate::error::Result;

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

    /// Creates a new kernel driver service with the given name and driver path.
    ///
    /// If the service already exists, it will be opened instead.
    ///
    /// # Panics
    ///
    /// Panics if the driver path is not valid UTF-8.
    pub fn create_service(&self, name: &str, path: &Path) -> Result<Service> {
        let service_name = CString::new(name)?;
        let driver_path = CString::new(path.to_str().unwrap())?;

        let service = unsafe {
            CreateServiceA(
                self.handle,
                PCSTR(service_name.as_ptr() as _),
                PCSTR(service_name.as_ptr() as _),
                SERVICE_ALL_ACCESS,
                SERVICE_KERNEL_DRIVER,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_IGNORE,
                PCSTR(driver_path.as_ptr() as _),
                None,
                None,
                None,
                None,
                None,
            )
        };

        match service {
            Ok(handle) => Ok(Service::new(handle)),
            Err(err) => {
                // Open the service if it already exists.
                if err.code() == ERROR_SERVICE_EXISTS.to_hresult() {
                    return self.open_service(name);
                }

                Err(err.into())
            }
        }
    }

    /// Opens an existing service by its name.
    pub fn open_service(&self, name: &str) -> Result<Service> {
        let service_name = CString::new(name)?;

        let handle = unsafe {
            OpenServiceA(
                self.handle,
                PCSTR(service_name.as_ptr() as _),
                SERVICE_ALL_ACCESS,
            )
        }?;

        Ok(Service::new(handle))
    }
}

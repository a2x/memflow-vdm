use std::path::Path;

use windows::core::PCSTR;
use windows::Win32::Foundation::{ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_EXISTS};
use windows::Win32::System::Services::*;

use crate::error::{Error, Result};

/// Loads a Windows Kernel Driver by creating a new service and then starting it.
///
/// # Arguments
///
/// * `driver_path` - The path to the driver file on disk (This file must exist).
/// * `service_name` - The name of the driver service.
///
/// # Returns
///
/// Returns `Ok(())` if the driver was successfully loaded, otherwise an error is returned.
///
/// # Remarks
///
/// This function requires administrative privileges in order to work.
pub fn load_driver<P: AsRef<Path>>(driver_path: P, service_name: &str) -> Result<()> {
    let driver_path = driver_path.as_ref();

    if !driver_path.exists() {
        return Err(Error::DriverNotFound);
    }

    let service_mgr = unsafe { OpenSCManagerA(None, None, SC_MANAGER_CREATE_SERVICE)? };

    let driver_path_ptr = PCSTR(format!("{}\0", driver_path.to_str().unwrap()).as_ptr());
    let service_name_ptr = PCSTR(format!("{}\0", service_name).as_ptr());

    let service = unsafe {
        CreateServiceA(
            service_mgr,
            service_name_ptr,
            service_name_ptr,
            SERVICE_ALL_ACCESS,
            SERVICE_KERNEL_DRIVER,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_IGNORE,
            driver_path_ptr,
            None,
            None,
            None,
            None,
            None,
        )
    };

    let service = match service {
        Ok(service) => service,
        Err(e) if e.code() == ERROR_SERVICE_EXISTS.to_hresult() => {
            // Open the existing driver service.
            unsafe { OpenServiceA(service_mgr, service_name_ptr, SERVICE_ALL_ACCESS) }
                .expect("unable to open driver service")
        }
        Err(e) => {
            _ = unsafe { CloseServiceHandle(service_mgr) };

            return Err(Error::Windows(e));
        }
    };

    let result = unsafe { StartServiceA(service, None) };

    if let Err(e) = result {
        // Check if the service is already running.
        if e.code() != ERROR_SERVICE_ALREADY_RUNNING.to_hresult() {
            unsafe {
                _ = DeleteService(service);
                _ = CloseServiceHandle(service);
                _ = CloseServiceHandle(service_mgr);
            }

            return Err(Error::Windows(e));
        }
    }

    unsafe {
        _ = CloseServiceHandle(service);
        _ = CloseServiceHandle(service_mgr);
    }

    Ok(())
}

/// Unloads a running Windows Kernel Driver by stopping the associated service and then deleting it.
///
/// # Arguments
///
/// * `driver_path` - The path to the driver file on disk (This file must exist).
/// * `service_name` - The name of the driver service.
///
/// # Returns
///
/// Returns `Ok(())` if the driver was successfully unloaded, otherwise an error is returned.
///
/// # Remarks
///
/// This function requires administrative privileges in order to work.
pub fn unload_driver<P: AsRef<Path>>(driver_path: P, service_name: &str) -> Result<()> {
    let driver_path = driver_path.as_ref();

    if !driver_path.exists() {
        return Err(Error::DriverNotFound);
    }

    let service_mgr = unsafe { OpenSCManagerA(None, None, SC_MANAGER_CREATE_SERVICE)? };

    let service = unsafe {
        OpenServiceA(
            service_mgr,
            PCSTR(format!("{}\0", service_name).as_ptr()),
            SERVICE_ALL_ACCESS,
        )
    };

    let service = match service {
        Ok(service) => service,
        Err(e) => {
            _ = unsafe { CloseServiceHandle(service_mgr) };

            return Err(Error::Windows(e));
        }
    };

    let mut status = SERVICE_STATUS::default();

    let result = unsafe { ControlService(service, SERVICE_CONTROL_STOP, &mut status) };

    if let Err(e) = result {
        unsafe {
            _ = CloseServiceHandle(service);
            _ = CloseServiceHandle(service_mgr);
        }

        return Err(Error::Windows(e));
    }

    let result = unsafe { DeleteService(service) };

    if let Err(e) = result {
        unsafe {
            _ = CloseServiceHandle(service);
            _ = CloseServiceHandle(service_mgr);
        }

        return Err(Error::Windows(e));
    }

    unsafe {
        _ = CloseServiceHandle(service);
        _ = CloseServiceHandle(service_mgr);
    }

    Ok(())
}

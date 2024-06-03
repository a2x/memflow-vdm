use std::path::Path;

use windows::core::PCSTR;
use windows::Win32::Foundation::{ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_EXISTS};
use windows::Win32::System::Services::*;

use crate::error::{Error, Result};

pub unsafe fn load_driver<P: AsRef<Path>>(driver_path: P, service_name: &str) -> Result<()> {
    let driver_path = driver_path.as_ref();

    if !driver_path.exists() {
        return Err(Error::DriverNotFound);
    }

    let service_mgr = OpenSCManagerA(None, None, SC_MANAGER_CREATE_SERVICE)?;

    let driver_path_ptr = PCSTR(format!("{}\0", driver_path.to_str().unwrap()).as_ptr());
    let service_name_ptr = PCSTR(format!("{}\0", service_name).as_ptr());

    let service = match CreateServiceA(
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
    ) {
        Ok(service) => service,
        Err(e) if e.code() == ERROR_SERVICE_EXISTS.to_hresult() => {
            // Open the existing service.
            OpenServiceA(service_mgr, service_name_ptr, SERVICE_ALL_ACCESS)?
        }
        Err(e) => {
            _ = CloseServiceHandle(service_mgr);

            return Err(Error::Windows(e));
        }
    };

    if let Err(e) = StartServiceA(service, None) {
        if e.code() != ERROR_SERVICE_ALREADY_RUNNING.to_hresult() {
            _ = DeleteService(service);

            _ = CloseServiceHandle(service);
            _ = CloseServiceHandle(service_mgr);

            return Err(Error::Windows(e));
        }
    }

    CloseServiceHandle(service)?;
    CloseServiceHandle(service_mgr)?;

    Ok(())
}

pub unsafe fn unload_driver<P: AsRef<Path>>(driver_path: P, service_name: &str) -> Result<()> {
    let driver_path = driver_path.as_ref();

    if !driver_path.exists() {
        return Err(Error::DriverNotFound);
    }

    let service_mgr = OpenSCManagerA(None, None, SC_MANAGER_CREATE_SERVICE)?;

    let service = match OpenServiceA(
        service_mgr,
        PCSTR(format!("{}\0", service_name).as_ptr()),
        SERVICE_ALL_ACCESS,
    ) {
        Ok(service) => service,
        Err(e) => {
            _ = CloseServiceHandle(service_mgr);

            return Err(Error::Windows(e));
        }
    };

    let mut status = SERVICE_STATUS::default();

    if let Err(e) = ControlService(service, SERVICE_CONTROL_STOP, &mut status) {
        _ = CloseServiceHandle(service);
        _ = CloseServiceHandle(service_mgr);

        return Err(Error::Windows(e));
    }

    if let Err(e) = DeleteService(service) {
        _ = CloseServiceHandle(service);
        _ = CloseServiceHandle(service_mgr);

        return Err(Error::Windows(e));
    }

    CloseServiceHandle(service)?;
    CloseServiceHandle(service_mgr)?;

    Ok(())
}

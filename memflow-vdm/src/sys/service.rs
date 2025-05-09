use windows::Win32::System::Services::*;
use windows::core::Result;

/// The current state of a Windows service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    Unknown = 0,
    Stopped = 1,
    StartPending = 2,
    StopPending = 3,
    Running = 4,
    ContinuePending = 5,
    PausePending = 6,
    Paused = 7,
}

impl ServiceState {
    /// Converts a raw [`SERVICE_STATUS_CURRENT_STATE`] value to a [`ServiceState`].
    #[must_use]
    pub fn from_raw(state: SERVICE_STATUS_CURRENT_STATE) -> ServiceState {
        match state {
            SERVICE_STOPPED => ServiceState::Stopped,
            SERVICE_START_PENDING => ServiceState::StartPending,
            SERVICE_STOP_PENDING => ServiceState::StopPending,
            SERVICE_RUNNING => ServiceState::Running,
            SERVICE_CONTINUE_PENDING => ServiceState::ContinuePending,
            SERVICE_PAUSE_PENDING => ServiceState::PausePending,
            SERVICE_PAUSED => ServiceState::Paused,
            _ => ServiceState::Unknown,
        }
    }
}

/// A created or queried Windows service.
pub struct Service {
    handle: SC_HANDLE,
}

impl Service {
    /// Creates a new [`Service`] with the given handle.
    pub fn new(handle: SC_HANDLE) -> Self {
        Service { handle }
    }

    /// Starts the service.
    ///
    /// This will query the current state of the service and will only attempt to start it if it's
    /// not already running.
    pub fn start(&self) -> Result<()> {
        let state = self.query_state()?;

        if state == ServiceState::Running {
            return Ok(());
        }

        unsafe { StartServiceA(self.handle, None) }
    }

    /// Stops the service by sending a stop control signal.
    ///
    /// The service must have the `SERVICE_STOP` access right for this to succeed.
    pub fn stop(&self) -> Result<()> {
        let mut status = SERVICE_STATUS::default();

        unsafe { ControlService(self.handle, SERVICE_CONTROL_STOP, &mut status) }
    }

    /// Deletes the service from the service control manager database.
    ///
    /// The service must be stopped first using [`stop`](Self::stop).
    ///
    /// Deletion is deferred until all open handles are closed, which happens when `self` is
    /// dropped.
    pub fn delete(&self) -> Result<()> {
        unsafe { DeleteService(self.handle) }
    }

    /// Queries the current state of the service.
    pub fn query_state(&self) -> Result<ServiceState> {
        let mut status = SERVICE_STATUS::default();

        unsafe {
            QueryServiceStatus(self.handle, &mut status)?;
        }

        Ok(ServiceState::from_raw(status.dwCurrentState))
    }
}

unsafe impl Send for Service {}
unsafe impl Sync for Service {}

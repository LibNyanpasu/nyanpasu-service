#![allow(dead_code)]

use nyanpasu_utils::runtime::block_on;
use std::{ffi::OsString, io::Result, sync::OnceLock, time::Duration};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
};

use crate::{
    consts::SERVICE_LABEL,
    utils::{os::register_ctrlc_handler, register_panic_hook},
};

pub fn run() -> Result<()> {
    service_dispatcher::start(SERVICE_LABEL, ffi_service_main)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

define_windows_service!(ffi_service_main, service_main);

pub fn service_main(args: Vec<OsString>) {
    if let Err(e) = run_service(args) {
        panic!("Error starting service: {:?}", e);
    }
    block_on(crate::handler());
}

static HANDLE_GUARD: OnceLock<ServiceHandleGuard> = OnceLock::new();
struct ServiceHandleGuard(ServiceStatusHandle);
impl Drop for ServiceHandleGuard {
    fn drop(&mut self) {
        let _ = self.0.set_service_status(ServiceStatus {
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
            service_type: ServiceType::OWN_PROCESS,
        });
    }
}

pub fn run_service(_arguments: Vec<OsString>) -> windows_service::Result<()> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate | ServiceControl::Stop => {
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    // Register system service event handler
    let status_handle = service_control_handler::register(SERVICE_LABEL, event_handler)?;

    let pid = std::process::id();
    let next_status = ServiceStatus {
        // Should match the one from system service registry
        service_type: ServiceType::OWN_PROCESS,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
        process_id: Some(pid),
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let _ = HANDLE_GUARD.set(ServiceHandleGuard(status_handle));

    // Do some work
    Ok(())
}
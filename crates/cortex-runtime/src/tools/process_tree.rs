//! Windows job ownership for shell descendants, including orphaned children.

use cortex_core::{CortexError, Result};
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub(super) struct ProcessTree(HANDLE);

impl ProcessTree {
    pub(super) fn new() -> Result<Self> {
        // The job handle is owned solely by this guard and never inherited.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(error("create process job"));
        }
        let job = Self(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let success = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if success == 0 {
            return Err(error("configure process job"));
        }
        Ok(job)
    }

    pub(super) fn attach(&self, child: &Child) -> Result<()> {
        if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle()) } == 0 {
            return Err(error("attach shell to process job"));
        }
        Ok(())
    }

    pub(super) fn terminate(&self) -> Result<()> {
        if unsafe { TerminateJobObject(self.0, 1) } == 0 {
            return Err(error("terminate shell process job"));
        }
        Ok(())
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        // KILL_ON_JOB_CLOSE covers descendants even if the direct shell exited.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn error(action: &str) -> CortexError {
    CortexError::Internal(format!(
        "failed to {action}: {}",
        std::io::Error::last_os_error()
    ))
}

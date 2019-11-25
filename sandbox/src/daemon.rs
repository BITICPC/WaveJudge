use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use nix::sys::signal::Signal;
use nix::sys::wait::{WaitStatus, WaitPidFlag};
use nix::unistd::Pid;

use super::{
    Error,
    ErrorKind,
    Result,
    ProcessResourceLimits,
    ProcessResourceUsage,
    ProcessExitStatus,
};

/// Provide a RAII guard type for safely waiting for `pid`s.
///
/// This type ensures that the child process is correcly waited for. If any
/// error occurs before the wait guard receives any status indicating the
/// process has exited (either normally or abnormally), the guard will kill the
/// child process when the guard instance is dropped.
struct WaitPidGuard {
    /// The pid of the process to wait on.
    pid: Pid,

    /// Whether the process should be killed when this instance is dropped.
    kill: bool
}

impl WaitPidGuard {
    /// Create a new `WaitPidGuard` instance.
    pub fn new(pid: Pid) -> Self {
        WaitPidGuard {
            pid,
            kill: true
        }
    }

    /// Wait for the child process. If a status indicating the child process
    /// has exited, then the guard will be released (it will not try to kill
    /// the child process when it is dropped).
    pub fn wait(&mut self, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        let wait_res = nix::sys::wait::waitpid(self.pid, options);
        match wait_res {
            Ok(WaitStatus::Exited(..)) | Ok(WaitStatus::Signaled(..)) => {
                self.kill = false;
                wait_res
            },
            Ok(status) => Ok(status),
            Err(e) => Err(e)
        }
    }
}

impl Drop for WaitPidGuard {
    fn drop(&mut self) {
        if self.kill {
            nix::sys::signal::kill(self.pid, Signal::SIGKILL)
                .expect("cannot kill the child process in the WaitPidGuard.");
        }
    }
}

/// Type for the join handle of the daemon thread.
pub type DaemonThreadJoinHandle = JoinHandle<()>;

/// Provide context information used in the daemon thread.
pub struct ProcessDaemonContext {
    /// The pid of the child process.
    pid: Pid,

    /// Process resource limits that should be implemented in the daemon thread.
    limits: Option<ProcessResourceLimits>,

    /// Status of the sandboxed child process.
    status: Mutex<ProcessExitStatus>,

    /// Resource usage statistics of the child process.
    rusage: Mutex<Option<ProcessResourceUsage>>,
}

impl ProcessDaemonContext {
    /// Create a new `ProcessDaemonContext` instance.
    pub fn new(pid: Pid, limits: Option<ProcessResourceLimits>) -> ProcessDaemonContext {
        ProcessDaemonContext {
            pid,
            limits,
            status: Mutex::new(ProcessExitStatus::NotExited),
            rusage: Mutex::new(None)
        }
    }

    /// Get the exit status stored in the context.
    pub fn exit_status(&self) -> ProcessExitStatus {
        self.status.lock().unwrap().clone()
    }

    /// Get the resource usage statistics stored in the context.
    pub fn rusage(&self) -> Option<ProcessResourceUsage> {
        *self.rusage.lock().unwrap()
    }
}

/// Checks that child process does not exceed daemon implemented limits.
fn daemon_check_limits(limits: &ProcessResourceLimits, usage: &ProcessResourceUsage,
    real_time_elapsed: Duration) -> Option<ProcessExitStatus> {
    let cpu_time_limit = limits.cpu_time_limit;
    if cpu_time_limit.is_some() && usage.cpu_time() > cpu_time_limit.unwrap() {
        return Some(ProcessExitStatus::CPUTimeLimitExceeded);
    }

    let real_time_limit = limits.real_time_limit;
    if real_time_limit.is_some() {
        if real_time_elapsed > real_time_limit.unwrap() {
            return Some(ProcessExitStatus::RealTimeLimitExceeded);
        }
    }

    let memory_limit = limits.memory_limit;
    if memory_limit.is_some() && usage.virtual_mem_size > memory_limit.unwrap() {
        return Some(ProcessExitStatus::MemoryLimitExceeded);
    }

    None
}

/// Get resource usage statistics for the given process and update the (maybe) existing one. Returns
/// the newest resource usage statistics.
fn daemon_update_rusage(pid: Pid, old: &mut Option<ProcessResourceUsage>)
    -> Result<ProcessResourceUsage> {
    let current_rusage = ProcessResourceUsage::usage_of(pid)?;
    match old {
        Some(ref mut old) => old.update(&current_rusage),
        None => *old = Some(current_rusage)
    };

    Ok(*old.as_ref().unwrap())
}

/// Main entry point of the daemon thread.
///
/// This function should not return `Ok(ProcessExitStatus::SandboxError)`. Instead, it should return
/// `Err(e)` with `e` set to the corresponding error.
fn daemon_main(context: &ProcessDaemonContext) -> Result<ProcessExitStatus> {
    // Interval between consecutive `wait` calls in the daemon thread.
    const WAIT_INTERVAL: Duration = Duration::from_millis(10);

    let mut wait_guard = WaitPidGuard::new(context.pid);

    // If we have daemon implemented resource constraits, then we should call `wait` with `WNOHANG`
    // flag; otherwise we should call `wait` without any flags.
    let wait_flag = context.limits.as_ref().and(Some(WaitPidFlag::WNOHANG));
    let has_daemon_limits = context.limits.is_some();

    // `timer` is used to measure elapsed real time.
    let timer = SystemTime::now();

    loop {
        trace!("Daemon calling wait...");
        let wait_status = wait_guard.wait(wait_flag)?;
        trace!("Daemon loop with wait status: {:?}", wait_status);

        match wait_status {
            WaitStatus::Exited(_, exit_code) =>
                return Ok(ProcessExitStatus::Normal(exit_code)),
            WaitStatus::Signaled(_, Signal::SIGSYS, _) =>
                return Ok(ProcessExitStatus::BannedSyscall),
            WaitStatus::Signaled(_, Signal::SIGUSR1, _) =>
                return Err(Error::from(ErrorKind::ChildStartupFailed)),
            WaitStatus::Signaled(_, sig, _) =>
                return Ok(ProcessExitStatus::KilledBySignal(sig)),
            _ => ()
        };

        // Collect process resource usage statistics.
        let overall_usage = daemon_update_rusage(context.pid,
            &mut *context.rusage.lock().unwrap())?;

        trace!("Daemon updated resource usage: {:?}", overall_usage);

        if has_daemon_limits {
            // Checks current usage statistics against the pre-set limits.
            let daemon_limits = context.limits.as_ref().unwrap();
            match daemon_check_limits(
                daemon_limits,
                &overall_usage,
                timer.elapsed().unwrap_or_default()) {
                Some(status) => return Ok(status),
                _ => ()
            };

            // Sleep for `WAIT_INTERVAL` milliseconds until the next `wait` call.
            std::thread::sleep(WAIT_INTERVAL);
        }
    }
}

/// Start the daemon thread. The daemon thread will monitor the process with the pid stored in the
/// given context. This function returns a `JoinHandle` instance representing a handle to the daemon
/// thread.
pub fn start(context: Arc<Box<ProcessDaemonContext>>) -> DaemonThreadJoinHandle {
    trace!("Starting daemon thread...");
    std::thread::spawn(move || {
        let exit_status = match daemon_main(&**context) {
            Ok(exit_status) => exit_status,
            Err(e) => panic!("daemon error: {}", e)
        };
        *(*context).status.lock().unwrap() = exit_status;
    })
}

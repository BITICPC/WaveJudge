//! This module contains facilities that relate to the seccomp feature of Linux
//! systems. This module is based on the `seccomp_sys` crate which furthur
//! depends on the `libseccomp` native library.
//!
//! Use `apply_syscall_blacklist` function to apply a blacklist of syscalls to
//! the calling process.
//!

use std::error::Error;
use std::fmt::{Display, Formatter};

use seccomp_sys::*;


/// The error type used in `seccomp` module.
#[derive(Clone, Copy, Debug)]
pub struct SeccompError {
    errno: i32
}

impl SeccompError {
    /// Create a new `SeccompError` instance.
    pub fn new(errno: i32) -> Self {
        SeccompError { errno }
    }

    /// Get the error number returned by the underlying `libseccomp` library.
    pub fn errno(&self) -> i32 {
        self.errno
    }
}

impl Display for SeccompError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("seccomp error: {}", self.errno))
    }
}

impl Error for SeccompError {
    // Use default trait implementation here.
}

/// The result type used in `seccomp` module.
pub type Result<T> = std::result::Result<T, SeccompError>;

/// Represent the action to take on specific syscall.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum Action {
    /// Allow the syscall.
    Allow,

    /// Kill the calling thread immediately.
    KillThread,

    /// Kill the calling process immediately, as though it is killed by the delivery of a `SIGSYS`
    /// signal.
    KillProcess,

    /// Send a `SIGSYS` signal to the calling thread.
    Trap,

    /// The called syscall immediately returns with the specified return value.
    Errno(u32),

    /// Notifying any tracing thread with the specified value.
    Trace(u32),
}

impl Action {
    /// Convert the `Action` enum value into native, libseccomp compatible format.
    pub fn as_native(&self) -> u32 {
        match self {
            Action::Allow => SCMP_ACT_ALLOW,
            Action::KillThread => SCMP_ACT_KILL,
            Action::KillProcess => SCMP_ACT_KILL_PROCESS,
            Action::Trap => SCMP_ACT_TRAP,
            Action::Errno(errno) => SCMP_ACT_ERRNO(*errno),
            Action::Trace(sig) => SCMP_ACT_TRACE(*sig)
        }
    }
}

/// Represent a syscall filter.
#[derive(Clone, Copy, Debug)]
pub struct SyscallFilter {
    /// The syscall ID to filter.
    pub syscall: i32,

    /// The action to perform when the specified syscall is called.
    pub action: Action
}

impl SyscallFilter {
    /// Create a new `SyscallFilter` value filtering on the given syscall with the given filter
    /// action.
    pub fn new(syscall: i32, action: Action) -> Self {
        SyscallFilter { syscall, action }
    }
}

/// Apply a list of syscall filters to the calling process. After calling this function, if the
/// calling process calls any of the syscalls not on the given list, then the kernel will kill the
/// calling process immediately; otherwise the corresponding action to the syscall will be
/// performed.
pub fn apply_syscall_filters<T>(filters: T) -> Result<()>
    where T: IntoIterator<Item = SyscallFilter>, {
    // TODO: Change the default behavior here to `SCMP_ACT_KILL_PROCESS` after upgrading to
    // TODO: Linux kernel 4.14 or above versions.
    let ctx = unsafe { seccomp_init(SCMP_ACT_KILL) };
    if ctx.is_null() {
        return Err(SeccompError::new(-1));
    }

    for filter in filters {
        let ret = unsafe {
            seccomp_rule_add_array(
                ctx, filter.action.as_native(), filter.syscall, 0, std::ptr::null())
        };
        if ret < 0 {
            return Err(SeccompError::new(ret));
        }
    }

    let ret = unsafe { seccomp_load(ctx) };
    if ret < 0 {
        return Err(SeccompError::new(ret));
    }

    Ok(())
}

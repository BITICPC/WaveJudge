//! This module provide Rust friendly bindings to the native `rlimit` mechanism.
//!

use libc::rlimit;

/// Represent a resource.
#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum Resource {
    /// Maximum size of the process's virtual memory (address space). This
    /// variant corresponds to the `RLIMIT_AS` native constant.
    AddressSpace = libc::RLIMIT_AS,

    /// Limit, in seconds, on the amount of CPU time that the process can
    /// consume. This variant corresponds to the `RLIMIT_CPU` native constant.
    CPUTime = libc::RLIMIT_CPU
}

/// Specify the soft limit and the hard limit for some resource.
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimit {
    /// The soft limit of the resource.
    pub soft_limit: u64,

    /// The hard limit of the resource.
    pub hard_limit: u64
}

impl ResourceLimit {
    /// Convert the `ResourceLimit` structure into native representation.
    fn as_native(&self) -> rlimit {
        rlimit {
            rlim_cur: self.soft_limit,
            rlim_max: self.hard_limit
        }
    }
}

/// Set resource limit for the calling process, using the native `rlimit` mechanism.
pub fn setrlimit(resource: Resource, limit: &ResourceLimit) -> std::io::Result<()> {
    let ret = unsafe { libc::setrlimit(resource as u32, &limit.as_native()) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Set resource limit for the calling process. The soft limit and the hard
/// limit are both set to the given `limit` value.
pub fn setrlimit_hard(resource: Resource, limit: u64) -> std::io::Result<()> {
    setrlimit(resource, &ResourceLimit {
        soft_limit: limit,
        hard_limit: limit
    })
}

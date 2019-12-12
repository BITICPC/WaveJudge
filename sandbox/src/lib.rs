//! This crate implements a sandbox for the judge. The sandbox is responsible
//! for executing tasks in a safe and monitored environment.
//!
//! The sandbox implements:
//!
//! * Normal process operations: create, start, monitor and kill a process;
//!
//! * Resource limits: CPU time limits, real time limits and memory limits;
//!
//! * Redirections: redirects stdin, stdout and stderr of child processes to
//! specific file descriptors;
//!
//! * Process syscall filter: filter out unexpected syscalls by seccomp feature.
//!

#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate nix;
extern crate seccomp_sys;
extern crate procinfo;

#[cfg(feature = "serde")]
extern crate serde;


mod daemon;
mod seccomp;
mod misc;
mod rlimits;

use std::cmp::Ordering;
use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::AsRawFd;

use nix::sys::signal::Signal;
use nix::unistd::{Uid, Pid, ForkResult};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use daemon::{ProcessDaemonContext, DaemonThreadJoinHandle};
use rlimits::Resource;

error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        Io(::std::io::Error);
        Nix(::nix::Error);
        Seccomp(seccomp::SeccompError);
    }

    errors {
        InvalidProcessArgument {
            description("invalid argument to subprocess")
        }

        InvalidEnvironmentVariable {
            description("invalid environment variable to subprocess")
        }

        InvalidSystemCallName {
            description("invalid system call name")
        }

        ChildStartupFailed {
            description("failed to launch child process")
        }

        DaemonFailed {
            description("daemon thread failed")
        }
    }
}


/// Measurement of the size of a block of memory.
#[derive(Clone, Copy, Debug, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MemorySize {
    /// Measurement in bytes.
    Bytes(usize),

    /// Measurement in kilobytes.
    KiloBytes(usize),

    /// Measurement in megabytes.
    MegaBytes(usize),

    /// Measurement in gigabytes.
    GigaBytes(usize),

    /// Measurement in terabytes.
    TeraBytes(usize)
}

impl MemorySize {
    /// Convert the current measurement to memory size in bytes.
    pub fn bytes(&self) -> usize {
        match self {
            MemorySize::Bytes(s) => *s,
            MemorySize::KiloBytes(s) => s * 1024,
            MemorySize::MegaBytes(s) => s * 1024 * 1024,
            MemorySize::GigaBytes(s) => s * 1024 * 1024 * 1024,
            MemorySize::TeraBytes(s) => s * 1024 * 1024 * 1024 * 1024
        }
    }
}

impl PartialEq for MemorySize {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for MemorySize {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MemorySize {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes().cmp(&other.bytes())
    }
}

impl From<usize> for MemorySize {
    fn from(value: usize) -> MemorySize {
        MemorySize::Bytes(value)
    }
}

impl Display for MemorySize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MemorySize::Bytes(s) => f.write_fmt(format_args!("{} B", s)),
            MemorySize::KiloBytes(s) => f.write_fmt(format_args!("{} KB", s)),
            MemorySize::MegaBytes(s) => f.write_fmt(format_args!("{} MB", s)),
            MemorySize::GigaBytes(s) => f.write_fmt(format_args!("{} GB", s)),
            MemorySize::TeraBytes(s) => f.write_fmt(format_args!("{} TB", s))
        }
    }
}

/// Type for representing system call IDs.
pub type SystemCallId = i32;

/// Represent a system call.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SystemCall {
    /// The name of the system call.
    pub name: String,

    /// The native ID of the system call.
    pub id: SystemCallId,

    /// This field is used to prevent external code from directly creating instances of
    /// `SystemCall`. External code is expected to create instances of `SystemCall` via `from_name`
    /// function.
    _msrnb: ()
}

impl SystemCall {
    /// Create a new `SystemCall` instance from a system call name. Returns
    /// `Err(ErrorKind::InvalidSystemCallName(..))` on failure.
    pub fn from_name<T>(name: T) -> Result<Self>
        where T: Into<String> {
        let name = name.into();
        let name_cstr = CString::new(name.clone())
            .map_err(|_| Error::from(ErrorKind::InvalidSystemCallName))?;

        let id = unsafe { seccomp_sys::seccomp_syscall_resolve_name(name_cstr.as_ptr()) };
        if id < 0 {
            debug!("Unknown syscall name: \"{}\"", name);
            return Err(Error::from(ErrorKind::InvalidSystemCallName));
        }

        Ok(SystemCall {
            name,
            id,
            _msrnb: ()
        })
    }
}

impl Display for SystemCall {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}({})", self.name, self.id))
    }
}

impl PartialEq for SystemCall {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Specify limits on time and memory resources.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProcessResourceLimits {
    /// Limit on CPU time available for the child process. `None` if no constraits are set.
    pub cpu_time_limit: Option<Duration>,

    /// Limit on real time available for the child process. `None` if no constraits are set.
    pub real_time_limit: Option<Duration>,

    /// Limit on memory available for the child process. `None` if no constraits are set.
    pub memory_limit: Option<MemorySize>
}

impl ProcessResourceLimits {
    /// Create a new `ProcessResourceLimits` instance that contains no constraits.
    fn empty() -> Self {
        ProcessResourceLimits {
            cpu_time_limit: None,
            real_time_limit: None,
            memory_limit: None
        }
    }
}

impl Default for ProcessResourceLimits {
    fn default() -> Self {
        ProcessResourceLimits::empty()
    }
}

/// Specify redirections of standard streams.
pub struct ProcessRedirection {
    /// Redirected `stdin`, or `None` if `stdin` does not need to be redirected.
    pub stdin: Option<File>,

    /// Redirected `stdout`, or `None` if `stdout` does not need to be
    /// redirected.
    pub stdout: Option<File>,

    /// Redirected `stderr`, or `None` if `stderr` does not need to be
    /// redirected.
    pub stderr: Option<File>
}

impl ProcessRedirection {
    /// Create a new `ProcessRedirection` instance representing that neither `stdin`, `stdout` nor
    /// `stderr` need to be redirected.
    fn empty() -> Self {
        ProcessRedirection {
            stdin: None,
            stdout: None,
            stderr: None
        }
    }
}

impl Default for ProcessRedirection {
    fn default() -> Self {
        ProcessRedirection::empty()
    }
}

/// Type for representing a user identification.
pub type UserId = u32;

/// Provide mechanism to build a child process in sandboxed environment.
pub struct ProcessBuilder {
    /// Path to the executable file.
    file: PathBuf,

    /// Arguments passed to the child process.
    args: Vec<String>,

    /// Environment variables passed to the child process.
    envs: Vec<(String, String)>,

    /// Working directory of the child process.
    pub working_dir: Option<PathBuf>,

    /// Limits to be applied to the new child process.
    pub limits: ProcessResourceLimits,

    /// Whether to use native rlimit mechanism to limit the resource usage of the child process. If
    /// you choose to use native rlimit mechanism, then the sandbox cannot report
    /// `TimeLimitExceeded` and `MemoryLimitExceeded` error, and the real time limit will not be
    /// applied.
    pub use_native_rlimit: bool,

    /// Redirections to be applied to the new child process.
    pub redirections: ProcessRedirection,

    /// Effective user ID of the new child process.
    pub uid: Option<UserId>,

    /// A list of allowed syscalls for the new child process.
    syscall_whitelist: Vec<SystemCall>,
}

impl ProcessBuilder {
    /// Create a new `ProcessBuilder` instance, given the executable file's path.
    pub fn new<T>(file: T) -> ProcessBuilder
        where T: Into<PathBuf> {
        let mut pb = ProcessBuilder {
            file: file.into(),
            args: Vec::with_capacity(1),
            envs: Vec::new(),
            working_dir: None,

            limits: ProcessResourceLimits::empty(),
            use_native_rlimit: false,
            redirections: ProcessRedirection::empty(),
            uid: None,

            syscall_whitelist: Vec::new()
        };

        // Add the path to the executable file as the first argument to the program.
        pb.args.push(pb.file.to_str().expect("file path to string failed").to_owned());

        pb
    }

    /// Add an argument to the child process. If the given argument is not a valid C-style string,
    /// then returns `Err(e)` where the error kind of `e` is `ErrorKind::InvalidProcessArgument`.
    pub fn add_arg<T>(&mut self, arg: T) -> Result<()>
        where T: Into<String> {
        let arg = arg.into();
        if misc::is_valid_c_string(&arg) {
            self.args.push(arg);
            Ok(())
        } else {
            debug!("Invalid process argument: \"{}\"", arg);
            bail!(ErrorKind::InvalidProcessArgument);
        }
    }

    /// Add an environment variable to the child process.
    pub fn add_env<T1, T2>(&mut self, name: T1, value: T2) -> Result<()>
        where T1: Into<String>, T2: Into<String> {
        let name = name.into();
        let value = value.into();

        if !misc::is_valid_c_string(&name) {
            debug!("Invalid environment variable name: \"{}\": not a valid C string.", name);
            bail!(ErrorKind::InvalidEnvironmentVariable);
        }
        if !misc::is_valid_c_string(&value) {
            debug!("Invalid environment variable value: \"{}\": not a valid C string.", value);
            bail!(ErrorKind::InvalidEnvironmentVariable);
        }
        if name.as_bytes().contains(&b'=') {
            debug!("Invalid environment variable name: \"{}\": contains a equal sign.", name);
            bail!(ErrorKind::InvalidEnvironmentVariable);
        }
        if value.as_bytes().contains(&b'=') {
            debug!("Invalid environment variable value: \"{}\": contains a equal sign.", value);
            bail!(ErrorKind::InvalidEnvironmentVariable);
        }

        self.envs.push((name, value));
        Ok(())
    }

    /// Add all environment variables in the calling process to the environment variables of the
    /// child process.
    pub fn inherit_env(&mut self) {
        for (name, value) in std::env::vars() {
            self.add_env(name, value)
                .expect("invalid environment variable in current process.");
        }
    }

    /// Mark the given syscall as allowed in the child process.
    pub fn allow_syscall(&mut self, sc: SystemCall) {
        self.syscall_whitelist.push(sc)
    }

    /// Apply working directory changes to the calling process.
    fn apply_working_directory(&self) -> Result<()> {
        if self.working_dir.is_some() {
            nix::unistd::chdir(self.working_dir.as_ref().unwrap().as_path())?;
        }

        Ok(())
    }

    /// Apply resource limits using native `rlimit` mechanism to the calling process.
    fn apply_native_rlimits(&self) -> Result<()> {
        if self.use_native_rlimit {
            if self.limits.cpu_time_limit.is_some() {
                rlimits::setrlimit_hard(Resource::CPUTime,
                    self.limits.cpu_time_limit.unwrap().as_secs())?;
            }
            if self.limits.memory_limit.is_some() {
                rlimits::setrlimit_hard(Resource::AddressSpace,
                    self.limits.memory_limit.unwrap().bytes() as u64)?;
            }
            // The real time limit is ignored here.
        }

        Ok(())
    }

    /// Apply redirections specified in `self.redirections` to the calling process.
    fn apply_redirections(&mut self) -> Result<()> {
        if self.redirections.stdin.is_some() {
            misc::dup_and_cloexec(
                self.redirections.stdin.as_ref().unwrap().as_raw_fd(),
                libc::STDIN_FILENO)?;
        }
        if self.redirections.stdout.is_some() {
            misc::dup_and_cloexec(
                self.redirections.stdout.as_ref().unwrap().as_raw_fd(),
                libc::STDOUT_FILENO)?;
        }
        if self.redirections.stderr.is_some() {
            misc::dup_and_cloexec(
                self.redirections.stderr.as_ref().unwrap().as_raw_fd(),
                libc::STDERR_FILENO)?;
        }

        Ok(())
    }

    /// Set the effective user ID stored in `self.uid` of the calling process.
    fn apply_uid(&self) -> Result<()> {
        if self.uid.is_some() {
            nix::unistd::setuid(Uid::from_raw(self.uid.unwrap()))?;
        }

        Ok(())
    }

    /// Apply seccomp to the calling process to filter syscall sequence.
    fn apply_seccomp(&self) -> Result<()> {
        if self.syscall_whitelist.is_empty() {
            return Ok(());
        }

        // If the child process calls syscalls that are not on the list of syscall whitelist, the
        // kernel will immediately kills the child process, as though it is been killed by the
        // delivery of a `SIGSYS` signal.
        seccomp::apply_syscall_filters(self.syscall_whitelist.iter()
            .map(|syscall| seccomp::SyscallFilter::new(syscall.id, seccomp::Action::Allow)))?;

        Ok(())
    }

    /// Start child process. This function will be called after `fork` in the child process. This
    /// function initializes necessary components in the child process (e.g. redirections, `setuid`,
    /// seccomp, etc.) and then calls `execve`.
    fn start_child(mut self) -> Result<()> {
        // TODO: Change the return type of this function to Result<!> after the `!` type stablizes.

        // Notes: No log messages are expected in the child process.

        // Build argv and envs into native format.
        let native_file = CString::new(Vec::from(self.file.as_os_str().as_bytes()))
            .unwrap();
        let native_argv = self.args.iter()
            .map(|arg| CString::new(arg.clone()).unwrap())
            .collect::<Vec<CString>>();
        let native_envs = self.envs.iter()
            .map(|env| format!("{}={}", env.0, env.1))
            .map(|env| CString::new(env).unwrap())
            .collect::<Vec<CString>>();

        // Apply redirections.
        self.apply_redirections()?;

        // Set current effective user ID if necessary.
        self.apply_uid()?;

        // Apply working directory changes.
        self.apply_working_directory()?;

        // Apply native resource limits.
        self.apply_native_rlimits()?;

        // Apply seccomp if necessary.
        self.apply_seccomp()?;

        // Finally, execve!
        nix::unistd::execve(&native_file, native_argv.as_ref(), native_envs.as_ref())?;

        unreachable!()
    }

    /// Initializes any necessary components in the parent process to monitor the states of the
    /// child process. This function should be called after `fork` in the parent process.
    fn start_parent(self, child_pid: Pid) -> Process {
        trace!("Starting parent process daemon...");

        let daemon_limits = if self.use_native_rlimit {
            None
        } else {
            Some(self.limits)
        };

        Process::attach(child_pid, daemon_limits)
    }

    /// Start the process in a sandboxed environment.
    pub fn start(self) -> Result<Process> {
        match nix::unistd::fork()? {
            ForkResult::Parent { child } => Ok(self.start_parent(child)),
            ForkResult::Child => {
                match self.start_child() {
                    Ok(..) => unreachable!(),
                    Err(e) => {
                        eprintln!("failed to start child process: {}", e);
                        // Send a `SIGUSR1` signal to self to terminate self and notify the daemon
                        // thread.
                        nix::sys::signal::kill(nix::unistd::getpid(), Signal::SIGUSR1)
                            .expect("cannot kill self.");
                        // Sit in a tight loop, wait to be killed by the delivery of the `SIGUSR1`
                        // signal whose default handling behavior is killing the target process.
                        loop { }
                    }
                }
            }
        }
    }
}

/// Type for the exit codes of processes.
pub type ProcessExitCode = i32;

/// Exit status of a sandboxed process.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ProcessExitStatus {
    /// The process has not exited yet.
    NotExited,

    /// The process exited normally.
    Normal(ProcessExitCode),

    /// The process was killed by the delivery of a signal.
    KilledBySignal(i32),

    /// The process was killed by the daemon due to CPU time limit.
    CPUTimeLimitExceeded,

    /// The process was killed by the daemon due to real time limit.
    RealTimeLimitExceeded,

    /// The process was killed by the daemon due to memory limit.
    MemoryLimitExceeded,

    /// The process was killed by the daemon due to its invocation to a banned
    /// system call.
    BannedSyscall
}

impl Default for ProcessExitStatus {
    fn default() -> Self {
        ProcessExitStatus::NotExited
    }
}

/// Resource usage statistics of a sandboxed process.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProcessResourceUsage {
    /// CPU time spent in user mode.
    pub user_cpu_time: Duration,

    /// CPU time spent in kernel mode.
    pub kernel_cpu_time: Duration,

    /// Virtual memory size.
    pub virtual_mem_size: MemorySize,

    /// Resident set size.
    pub resident_set_size: MemorySize
}

impl ProcessResourceUsage {
    /// Create an empty `ProcessResourceUsage` instance.
    pub fn new() -> Self {
        ProcessResourceUsage {
            user_cpu_time: Duration::new(0, 0),
            kernel_cpu_time: Duration::new(0, 0),
            virtual_mem_size: MemorySize::Bytes(0),
            resident_set_size: MemorySize::Bytes(0)
        }
    }

    /// Get resource usage for the specified process.
    pub fn usage_of(pid: Pid) -> std::io::Result<Self> {
        Ok(ProcessResourceUsage::from(procinfo::pid::stat(pid.as_raw())?))
    }

    /// Get the total CPU time consumed, a.k.a. the sum of the user CPU time and
    /// the kernel CPU time.
    pub fn cpu_time(&self) -> Duration {
        self.user_cpu_time + self.kernel_cpu_time
    }

    /// Update the usage statistics stored in this instance to the statistics
    /// stored in the given statistics.
    pub fn update(&mut self, other: &Self) {
        if other.user_cpu_time > self.user_cpu_time {
            self.user_cpu_time = other.user_cpu_time;
        }
        if other.kernel_cpu_time > self.kernel_cpu_time {
            self.kernel_cpu_time = other.kernel_cpu_time;
        }
        if other.virtual_mem_size > self.virtual_mem_size {
            self.virtual_mem_size = other.virtual_mem_size;
        }
        if other.resident_set_size > self.resident_set_size {
            self.resident_set_size = other.resident_set_size;
        }
    }
}

impl From<procinfo::pid::Stat> for ProcessResourceUsage {
    fn from(stat: procinfo::pid::Stat) -> Self {
        ProcessResourceUsage {
            user_cpu_time: misc::duration_from_clocks(stat.utime),
            kernel_cpu_time: misc::duration_from_clocks(stat.stime),
            virtual_mem_size: MemorySize::Bytes(stat.vsize),
            resident_set_size: MemorySize::Bytes(stat.rss)
        }
    }
}

impl Default for ProcessResourceUsage {
    fn default() -> Self {
        ProcessResourceUsage::new()
    }
}

/// Type for representing process IDs.
pub type ProcessId = i32;

/// A handle to the sandboxed child process.
pub struct Process {
    /// Pid of the child process.
    pid: Pid,

    /// Daemon related context.
    context: Arc<Box<ProcessDaemonContext>>,

    /// Join handle of the daemon thread. `None` if the `Process` instance has
    /// been waited for.
    daemon: Option<DaemonThreadJoinHandle>
}

impl Process {
    /// Create a new `Process` instance attaching to the specific process.
    fn attach(pid: Pid, limits: Option<ProcessResourceLimits>) -> Process {
        trace!("Process::attach to process ID {}", pid.as_raw());

        let mut handle = Process {
            pid,
            context: Arc::new(Box::new(ProcessDaemonContext::new(pid, limits))),
            daemon: None
        };

        let daemon_handle = daemon::start(handle.context.clone());
        trace!("Daemon thread started");
        handle.daemon = Some(daemon_handle);

        handle
    }

    /// Get the ID of the child process.
    pub fn pid(&self) -> ProcessId {
        self.pid.as_raw()
    }

    /// Get the exit status of the process.
    pub fn exit_status(&self) -> ProcessExitStatus {
        self.context.exit_status()
    }

    /// Get the resource usage statistics of the process.
    pub fn rusage(&self) -> ProcessResourceUsage {
        self.context.rusage()
            .unwrap_or_else(|| ProcessResourceUsage::new())
    }

    /// Wait for the child process to exit. Panics if this function has been
    /// called already on the same `Process` instance.
    pub fn wait_for_exit(&mut self) -> Result<()> {
        self.daemon.take().unwrap().join()
            .map_err(|_| Error::from(ErrorKind::DaemonFailed))
    }
}


#[cfg(test)]
mod tests {
    use super::MemorySize;

    #[test]
    fn test_memory_size_to_bytes() {
        assert_eq!(2, MemorySize::Bytes(2).bytes());
        assert_eq!(2 * 1024, MemorySize::KiloBytes(2).bytes());
        assert_eq!(2 * 1024 * 1024, MemorySize::MegaBytes(2).bytes());
        assert_eq!(2 * 1024 * 1024 * 1024, MemorySize::GigaBytes(2).bytes());
        assert_eq!(2 * 1024 * 1024 * 1024 * 1024, MemorySize::TeraBytes(2).bytes());
    }
}

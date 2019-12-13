//! This module implements some IO utilities used in the fork server.
//!

use std::ffi::CString;
use std::fs::File;
use std::os::unix::io::{FromRawFd};

/// Represents a pipe.
pub struct Pipe {
    /// The read end of the pipe.
    pub reader: File,

    /// The write end of the pipe.
    pub writer: File,
}

/// Create a new anonymous pipe.
pub fn create_pipe() -> nix::Result<Pipe> {
    let (reader_fd, writer_fd) = nix::unistd::pipe()?;
    Ok(Pipe {
        reader: unsafe { File::from_raw_fd(reader_fd) },
        writer: unsafe { File::from_raw_fd(writer_fd) }
    })
}

/// Get a mutable reference to `errno`.
fn get_errno_mut() -> &'static mut i32 {
    unsafe { libc::__errno_location().as_mut().unwrap() }
}

/// Get the value of `errno`.
fn get_errno() -> i32 {
    *get_errno_mut()
}

/// Test whether `errno` is set (non-zero).
fn has_errno() -> bool {
    get_errno() != 0
}

/// Set the value of `errno` to the given value.
fn set_errno(errno: i32) {
    *get_errno_mut() = errno;
}

/// Set the value of `errno` to 0.
fn clear_errno() {
    set_errno(0);
}

/// Lookup the password file and get the corresponding uid to the given username.
pub fn lookup_uid<T>(username: T) -> std::io::Result<Option<u32>>
    where T: AsRef<str> {
    let username = CString::new(username.as_ref())
        .expect("failed to create CString from the given username.");

    clear_errno();
    let pwd = unsafe {
        libc::getpwnam(username.as_ptr()).as_ref()
    };

    match pwd {
        Some(pwd) => Ok(Some(pwd.pw_uid)),
        None => {
            if has_errno() {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(None)
            }
        }
    }
}

//! This module defines IO related facilities used in the judge engine, such as
//! pipes.
//!

use std::fs::File;

use std::os::unix::io::{RawFd, FromRawFd};

use crate::Result;


/// Represent a pipe with a read end and a write end. The read end and the write
/// end of the pipe can be manipulated independently.
///
/// The first field of the tuple struct is the read end, the second field of the
/// tuple struct is the write end.
pub struct Pipe(pub Option<File>, pub Option<File>);

impl Pipe {
    /// Create a new `Pipe` instance.
    pub fn new() -> Result<Pipe> {
        let (read_fd, write_fd) = nix::unistd::pipe()?;
        Ok(Pipe::from_raw_fd(read_fd, write_fd))
    }

    /// Create a new `Pipe` instance whose 2 ends are constructed from raw file
    /// descriptors.
    pub fn from_raw_fd(read_fd: RawFd, write_fd: RawFd) -> Pipe {
        Pipe(
            Some(unsafe { File::from_raw_fd(read_fd) }),
            Some(unsafe { File::from_raw_fd(write_fd) })
        )
    }

    /// Get a reference to the read end of the pipe.
    pub fn read_end(&self) -> Option<&File> {
        self.0.as_ref()
    }

    /// Get a mutable reference to the read end of the pipe.
    pub fn read_end_mut(&mut self) -> Option<&mut File> {
        self.0.as_mut()
    }

    /// Get a reference to the write end of the pipe.
    pub fn write_end(&self) -> Option<&File> {
        self.1.as_ref()
    }

    /// Get a mutable reference to the write end of the pipe.
    pub fn write_end_mut(&mut self) -> Option<&mut File> {
        self.1.as_mut()
    }

    /// Take ownership of the read end of the pipe, leaving `None` in the
    /// corresponding slot in this `Pipe` instance.
    pub fn take_read_end(&mut self) -> Option<File> {
        self.0.take()
    }

    /// Take ownership of the write end of the pipe, leaving `None` in the
    /// corresponding slot in this `Pipe` instance.
    pub fn take_write_end(&mut self) -> Option<File> {
        self.1.take()
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        match self.take_read_end() {
            Some(file) => drop(file),
            None => ()
        };
        match self.take_write_end() {
            Some(file) => drop(file),
            None => ()
        }
    }
}

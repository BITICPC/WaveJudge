//! This module defines IO related facilities used in the judge engine, such as pipes.
//!

use std::fs::File;
use std::io::Read;
use std::path::Path;

use std::os::unix::io::{FromRawFd, AsRawFd};

use crate::Result;

/// Create a new pipe. The first field of the returned tuple is the read end of the pipe and the
/// second field of the returned tuple is the write end of the pipe.
pub fn pipe() -> Result<(File, File)> {
    let (read_fd, write_fd) = nix::unistd::pipe()?;
    Ok((
        unsafe { File::from_raw_fd(read_fd) },
        unsafe { File::from_raw_fd(write_fd) }
    ))
}

/// Provide a `read_token` method on `Read` taits where tokens are separated by blank characters.
pub trait TokenizedRead {
    /// Read next token from the underlying device. Tokens are separated by blank characters.
    fn read_token(&mut self) -> std::io::Result<Option<String>>;
}

/// Provide a default implementation of `TokenizedRead`.
pub struct TokenizedReader<R: Read> {
    /// The inner buffered reader.
    inner: R,

    /// Internal buffer holding bytes read from the inner reader.
    buffer: Vec<u8>,

    /// The number of available bytes currently in `buffer`.
    buffer_size: usize,

    /// The read head of this reader into the buffer.
    ptr: usize
}

impl<R: Read> TokenizedReader<R> {
    pub const BUFFER_SIZE: usize = 4096;

    /// Create a new `TokenizedReader` instance.
    pub fn new(inner: R) -> TokenizedReader<R> {
        TokenizedReader {
            inner,
            buffer: vec![0; TokenizedReader::<R>::BUFFER_SIZE],
            buffer_size: 0,
            ptr: 0
        }
    }

    /// Read next block of bytes into the internal buffer.
    fn read_block(&mut self) -> std::io::Result<()> {
        self.buffer_size = self.inner.read(self.buffer.as_mut())?;
        self.ptr = 0;
        Ok(())
    }

    /// Read a single byte from the underlying reader.
    ///
    /// This function returns `Ok(Some(..))` if one byte is successfully read, returns `Ok(None)` if
    /// EOF is hit, returns `Err(..)` on IO errors.
    fn read_byte(&mut self) -> std::io::Result<Option<u8>> {
        if self.ptr >= self.buffer_size {
            self.read_block()?;
            if self.ptr >= self.buffer_size {
                return Ok(None);
            }
        }

        let byte = self.buffer[self.ptr];
        self.ptr += 1;
        Ok(Some(byte))
    }
}

impl<R: Read> TokenizedRead for TokenizedReader<R> {
    fn read_token(&mut self) -> std::io::Result<Option<String>> {
        static SEPERATE_BYTES: &'static [u8] = &[b' ', b'\r', b'\n', b'\t'];

        // Skip any leading whitespace characters.
        let mut byte = SEPERATE_BYTES[0];
        while SEPERATE_BYTES.contains(&byte) {
            byte = match self.read_byte()? {
                Some(b) => b,
                None => return Ok(None)
            };
        }

        // First non-whitespace character has been hit and stored in `byte`.
        let mut buffer = Vec::<u8>::new();
        while !SEPERATE_BYTES.contains(&byte) {
            buffer.push(byte);
            byte = match self.read_byte()? {
                Some(b) => b,
                None => break
            };
        }

        let token = String::from_utf8(buffer)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))
            ?;
        Ok(Some(token))
    }
}

/// Check that the given `nix::Error` instance is a system error represented by
/// `nix::Error::Sys(..)` and returns the inner error number. Otherwise this function panics.
fn expect_nix_sys_err(err: nix::Error) -> i32 {
    match err {
        nix::Error::Sys(errno) => errno as i32,
        _ => panic!("unexpected nix error: {}", err)
    }
}

/// Provide extension functions to `Read`.
pub trait ReadExt {
    /// Read contents into a string, with a specified maximal length. Any non-UTF8 byte sequences
    /// in the data will be replaced by `U+FFFD Replacement Character`, which displays like `�`.
    fn read_to_string_lossy(&mut self, max_len: usize) -> std::io::Result<Option<String>>;
}

impl<T: Read> ReadExt for T {
    fn read_to_string_lossy(&mut self, max_len: usize) -> std::io::Result<Option<String>> {
        let mut buffer = vec![0u8; max_len];
        let bytes_read = self.read(&mut buffer)?;

        if bytes_read == 0 {
            Ok(None)
        } else {
            Ok(Some(String::from_utf8_lossy(&buffer[..bytes_read]).into_owned()))
        }
    }
}

/// Read a data view of the specified file with a maximal length.
pub fn read_file_view<P>(path: &P, max_len: usize) -> std::io::Result<String>
    where P: ?Sized + AsRef<Path> {
    let mut f = File::open(path)?;
    let view = f.read_to_string_lossy(max_len)?.unwrap_or_default();

    Ok(view)
}

/// Provide extension functions to `File`.
pub trait FileExt {
    /// Duplicate a `File` instance by duplicating its underlying file descriptor using the `dup`
    /// system call.
    fn duplicate(&self) -> std::io::Result<File>;
}

impl FileExt for File {
    fn duplicate(&self) -> std::io::Result<File> {
        let my_fd = self.as_raw_fd();
        let dup_fd = nix::unistd::dup(my_fd)
            .map_err(|e| std::io::Error::from_raw_os_error(expect_nix_sys_err(e)))
            ?;

        Ok(unsafe { File::from_raw_fd(dup_fd) })
    }
}

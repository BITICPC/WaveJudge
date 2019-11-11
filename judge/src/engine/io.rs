//! This module defines IO related facilities used in the judge engine, such as pipes.
//!

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

use std::os::unix::io::{RawFd, FromRawFd};

use crate::Result;


/// Represent a pipe with a read end and a write end. The read end and the write end of the pipe can
/// be manipulated independently.
///
/// The first field of the tuple struct is the read end, the second field of the tuple struct is the
/// write end.
pub struct Pipe(pub Option<File>, pub Option<File>);

impl Pipe {
    /// Create a new `Pipe` instance.
    pub fn new() -> Result<Pipe> {
        let (read_fd, write_fd) = nix::unistd::pipe()?;
        Ok(Pipe::from_raw_fd(read_fd, write_fd))
    }

    /// Create a new `Pipe` instance whose 2 ends are constructed from raw file descriptors.
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

    /// Take ownership of the read end of the pipe, leaving `None` in the corresponding slot in this
    /// `Pipe` instance.
    pub fn take_read_end(&mut self) -> Option<File> {
        self.0.take()
    }

    /// Take ownership of the write end of the pipe, leaving `None` in the corresponding slot in
    /// this `Pipe` instance.
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

/// Represent a temporary file.
pub struct TempFile {
    /// Path to the temporary file.
    pub path: PathBuf,

    /// The file object representing an opened handle to the temporary file.
    pub file: File
}

impl TempFile {
    /// Create a path template for creating temporary files. The returned path template can be
    /// passed to `mkstemp` native function to create temporary files.
    fn make_path_template() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(PathBuf::from_str("judge_temp_XXXXXX").unwrap());

        path
    }

    /// Create a new temporary file.
    pub fn new() -> std::io::Result<TempFile> {
        let path = TempFile::make_path_template();
        TempFile::from_path_template(path)
    }

    /// Create a new temporary file with the given path template. The given path template should be
    /// valid to be passed to the `mkstemp` native function to create temporary files; otherwise
    /// this function panics.
    pub fn from_path_template(template: PathBuf) -> std::io::Result<TempFile> {
        let (fd, path) = match nix::unistd::mkstemp(&template) {
            Ok(ret) => ret,
            Err(e) => match e {
                nix::Error::Sys(errno) =>
                    return Err(std::io::Error::from_raw_os_error(errno as i32)),
                _ => panic!("unexpected nix error in TempFile::new(): {}", e)
            }
        };

        Ok(TempFile {
            path,
            file: unsafe { File::from_raw_fd(fd) }
        })
    }
}

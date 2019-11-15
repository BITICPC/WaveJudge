//! This module implements a dynamic linking library loader for loading dylibs that contains
//! language providers.
//!
//! To make your dynamic linking library loadable by this loader implementation, your dylib must
//! contains an exported symbol `init_language_providers`, which should be a function written in
//! Rust. In this function, you register your language providers into the singleton
//! `LanguageManager` instance using the `register` function.
//!
//! The `init_language_providers` function in your dynamic linking library should has the following
//! signature:
//!
//! ```ignore
//! fn init_language_providers();
//! ```
//!
//! Otherwise the behavior is undefined.
//!

use std::fmt::{Display, Formatter};
use std::path::Path;

use libloading::{Library, Symbol};


/// Provide an error type used when loading external dynamic linking libraries.
#[derive(Debug)]
pub enum LoadDylibError {
    /// IO error.
    IoError(std::io::Error),

    /// Error raised by the external code inside the dynamic linking library.
    DylibError(Box<dyn std::error::Error>)
}

impl From<std::io::Error> for LoadDylibError {
    fn from(err: std::io::Error) -> Self {
        LoadDylibError::IoError(err)
    }
}

impl From<Box<dyn std::error::Error>> for LoadDylibError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        LoadDylibError::DylibError(err)
    }
}

impl Display for LoadDylibError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use LoadDylibError::*;
        match self {
            IoError(ref err) => f.write_fmt(format_args!("IO error: {}", err)),
            DylibError(ref err) => f.write_fmt(format_args!("dylib error: {}", err))
        }
    }
}

impl std::error::Error for LoadDylibError { }


/// Symbol name for the init function in the dynamic linking library.
const DYLIB_INIT_SYMBOL: &'static [u8] = b"init_language_providers\x00";

/// Type used to represent the primary load function inside a dynamic linking library containing
/// language providers.
type LoadFunction = unsafe extern fn() -> Result<(), Box<dyn std::error::Error>>;

/// Load the given dynamic linking library containing custom language providers into the
/// application.
pub fn load_dylib(file: &Path) -> Result<(), LoadDylibError> {
    let lib = Library::new(file)?;
    unsafe {
        let func: Symbol<LoadFunction> = lib.get(DYLIB_INIT_SYMBOL)?;
        func()?;
    }

    Ok(())
}

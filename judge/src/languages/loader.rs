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

use std::path::Path;

use libloading::{Library, Symbol};

use super::LanguageManager;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
    }

    errors {
        DylibError(message: String) {
            description("dylib error")
            display("dylib error: {}", message)
        }
    }
}


/// Provide implementation of a dylib loader for loading dynamic linkling libraries containing
/// language providers.
pub struct DylibLoader {
    /// Loaded dynamic libraries. Note that we must maintain loaded libraries inside this vector to
    /// prevent them from being dropped since `Library` objects automatically unload the
    /// corresponding dynamic library when they are dropped.
    loaded: Vec<Library>,
}

impl DylibLoader {
    /// Create a new `DylibLoader` object.
    pub fn new() -> Self {
        DylibLoader {
            loaded: Vec::new(),
        }
    }

    /// Load the specified library.
    pub fn load<P>(&mut self, file: &P, lang_mgr: &LanguageManager) -> Result<()>
        where P: ?Sized + AsRef<Path> {
        let file = file.as_ref();
        log::info!("Loading language provider library: \"{}\"...", file.display());

        let lib = Library::new(file)?;
        let func: Symbol<InitFunc> = match unsafe { lib.get(DYLIB_INIT_SYMBOL) } {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to load dylib: \"{}\": {}", file.display(), e);
                return Err(Error::from(ErrorKind::IoError(e)));
            }
        };

        match unsafe { func(lang_mgr) } {
            Ok(..) => (),
            Err(e) => {
                log::error!("dylib initialization failed: {}", e);
                return Err(Error::from(ErrorKind::DylibError(format!("{}", e))));
            }
        };

        self.loaded.push(lib);
        Ok(())
    }
}

/// Symbol name for the init function in the dynamic linking library.
const DYLIB_INIT_SYMBOL: &'static [u8] = b"init_language_providers\x00";

/// Type used to represent the primary load function inside a dynamic linking library containing
/// language providers.
type InitFunc = unsafe extern "Rust" fn(&LanguageManager)
    -> std::result::Result<(), Box<dyn std::error::Error>>;

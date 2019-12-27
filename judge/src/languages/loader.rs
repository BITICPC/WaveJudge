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

use super::LanguageProviderRegister;

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

/// Symbol name for the init function in the dynamic linking library.
const DYLIB_INIT_SYMBOL: &'static [u8] = b"init_language_providers\x00";

/// Type used to represent the primary load function inside a dynamic linking library containing
/// language providers.
type InitFunc = unsafe extern "Rust" fn(&mut LanguageProviderRegister)
    -> std::result::Result<(), Box<dyn std::error::Error>>;

/// Load the specified library.
pub fn load<P>(file: &P, lang_reg: &mut LanguageProviderRegister) -> Result<Library>
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

    match unsafe { func(lang_reg) } {
        Ok(..) => (),
        Err(e) => {
            log::error!("dylib initialization failed: {}", e);
            return Err(Error::from(ErrorKind::DylibError(
                format!("dylib initialization failed: {}", e))));
        }
    };

    Ok(lib)
}

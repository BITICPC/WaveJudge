//! This module implements a dynamic linking library loader for loading dylibs
//! that contains language providers.
//!
//! To make your dynamic linking library loadable by this loader implementation,
//! your dylib must contains an exported symbol `init_language_providers`,
//! which should be a function written in Rust. In this function, you register
//! your language providers into the singleton `LanguageManager` instance using
//! the `register` function.
//!
//! The `init_language_providers` function in your dynamic linking library
//! should has the following signature:
//!
//! ```ignore
//! fn init_language_providers();
//! ```
//!
//! Otherwise the behavior is undefined.
//!

use std::path::Path;

use libloading::{Library, Symbol};


/// Symbol name for the init function in the dynamic linking library.
const DYLIB_INIT_SYMBOL: &'static [u8] = b"init_language_providers";


/// Load the given dynamic linking library containing custom language providers
/// into the application.
pub fn load_dylib(file: &Path) -> std::io::Result<()> {
    let lib = Library::new(file)?;
    unsafe {
        let func: Symbol<unsafe extern fn()> = lib.get(DYLIB_INIT_SYMBOL)?;
        func();
    }

    Ok(())
}

//! This crate provides definitions for built-in language providers of wave judge system including:
//!
//! * C/C++ language provider, which resides in the `cxx` module;
//! * Rust language provider, which resides in the `rust` module;
//! * Java language provider, which resides in the `java` module;
//! * Python language provider, which resides in the `py` module.
//!
//! This crate is configured to be built into a dynamic linking library which will be explicitly
//! loaded by the judge program during startup. The `init_language_provider` function provides main
//! logic for loading contents of this crate into the judge system.
//!

extern crate judge;

mod cxx;

use std::fmt::{Display, Formatter};


/// Provide an error type that can be returned while initializing language providers.
#[derive(Debug)]
pub struct InitLanguageError {
    /// Error message.
    pub message: String
}

impl InitLanguageError {
    /// Create a new `InitLanguageError` instance.
    pub fn new<T: ToString>(message: T) -> Self {
        InitLanguageError {
            message: message.to_string()
        }
    }

    /// Box this instance and returns a `Box` instance.
    pub fn into_boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

impl Display for InitLanguageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InitLanguageError { }


/// This function is called by the judge loader to initialize and load available language providers
/// in this library.
pub extern "Rust" fn init_language_providers() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement init_language_providers.

    cxx::init_cxx_providers().map_err(|e| e.into_boxed())?;

    unimplemented!()
}

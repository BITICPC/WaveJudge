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

extern crate log;
extern crate serde;
extern crate serde_yaml;
extern crate judge;

mod cxx;
mod java;
mod py;
mod rust;
mod utils;

use std::fmt::{Display, Formatter};

use judge::languages::LanguageManager;


/// Provide an error type that can be returned while initializing language providers.
#[derive(Debug)]
pub struct InitLanguageError {
    /// Error message.
    pub message: String
}

impl InitLanguageError {
    /// Create a new `InitLanguageError` instance.
    pub fn new<T>(message: T) -> Self
        where T: Into<String> {
        InitLanguageError {
            message: message.into()
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

/// Provide a facade type for language provider initialization functions.
type BuiltinLanguageProviderInitializer = fn(&LanguageManager) -> Result<(), InitLanguageError>;

/// This function is called by the judge loader to initialize and load available language providers
/// in this library.
#[no_mangle]
pub extern "Rust" fn init_language_providers(lang: &LanguageManager)
    -> Result<(), Box<dyn std::error::Error>> {
    let initializers: [(&'static str, BuiltinLanguageProviderInitializer); 4] = [
        ("cxx", cxx::init_cxx_providers),
        ("java", java::init_java_providers),
        ("python", py::init_py_providers),
        ("rust", rust::init_rust_providers)
    ];

    for (name, init) in &initializers {
        log::info!("Initializing {} language providers...", name);
        match init(lang) {
            Ok(..) => (),
            Err(e) => {
                return Err(e.into_boxed());
            }
        }
    }

    Ok(())
}

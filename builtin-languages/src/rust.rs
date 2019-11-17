//! This module defines the language provider for the Rust programming language.
//!

use crate::InitLanguageError;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Once;

use judge::{
    Program,
    CompilationScheme
};
use judge::engine::{
    CompilationInfo,
    ExecutionInfo
};
use judge::languages::{
    LanguageBranch,
    LanguageProvider,
    LanguageProviderMetadata,
    LanguageManager,
    ExecutionScheme
};

static mut METADATA: Option<LanguageProviderMetadata> = None;
static METADATA_ONCE: Once = Once::new();

fn init_metadata() {
    METADATA_ONCE.call_once(|| {
        let mut metadata = LanguageProviderMetadata::new(String::from("rust"), false);
        metadata.branches.push(LanguageBranch::new("rust", "39"));

        unsafe {
            METADATA = Some(metadata);
        }
    });
}

const WAVETESTLIB_CRATE_DIR_ENV: &'static str = "WAVETESTLIB_CRATE_DIR";

/// Provide environment related information of Rust programming language.
struct RustEnvironment {
    /// Path to the directory containing the wave test lib crate.
    testlib_crate_dir: PathBuf
}

impl RustEnvironment {
    /// Create a new `RustEnvironment` instance whose information is collected from current
    /// context.
    fn new() -> Result<Self, InitLanguageError> {
        let testlib_crate_dir = std::env::var(WAVETESTLIB_CRATE_DIR_ENV)
            .map(|v| PathBuf::from_str(&v).unwrap())
            .map_err(|_| InitLanguageError::new(format!("Env variable \"{}\" not set.",
                WAVETESTLIB_CRATE_DIR_ENV)))
            ?;
        Ok(RustEnvironment { testlib_crate_dir })
    }
}

/// Language provider of the Rust programming language.
struct RustLanguageProvider {
    /// Rust language environment.
    env: RustEnvironment
}

impl RustLanguageProvider {
    /// Create a new `RustLanguageProvider` instance.
    fn new(env: RustEnvironment) -> Self {
        RustLanguageProvider { env }
    }
}

impl LanguageProvider for RustLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, output_dir: Option<&Path>, scheme: CompilationScheme)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        let output_file = crate::utils::make_output_file_path(&program.file, output_dir);

        let mut ci = CompilationInfo::new(&PathBuf::from("rustc"), &output_file);
        ci.compiler.args.push(String::from("-C"));
        ci.compiler.args.push(String::from("opt-level=2"));
        ci.compiler.args.push(String::from("--cfg"));
        ci.compiler.args.push(String::from("online_judge"));

        match scheme {
            CompilationScheme::Checker | CompilationScheme::Interactor => {
                ci.compiler.args.push(String::from("-L"));
                ci.compiler.args.push(String::from(self.env.testlib_crate_dir.to_str().unwrap()));
            },
            _ => ()
        };

        ci.compiler.args.push(String::from("-o"));
        ci.compiler.args.push(format!("\"{}\"", program.file.display()));

        Ok(ci)
    }

    fn execute(&self, program: &Program, _scheme: ExecutionScheme)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        Ok(ExecutionInfo::new(&program.file))
    }
}

/// Initialize language providers for the Rust programming language.
pub fn init_rust_providers() -> Result<(), InitLanguageError> {
    init_metadata();

    let env = RustEnvironment::new()?;

    let lang_mgr = LanguageManager::singleton();
    lang_mgr.register(Box::new(RustLanguageProvider::new(env)));

    Ok(())
}

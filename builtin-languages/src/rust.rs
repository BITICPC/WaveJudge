//! This module defines the language provider for the Rust programming language.
//!

use crate::InitLanguageError;

use std::path::PathBuf;
use std::sync::Once;

use serde::Deserialize;

use judge::{
    Program,
    ProgramKind,
};
use judge::languages::{
    LanguageBranch,
    LanguageProvider,
    LanguageProviderMetadata,
    LanguageProviderRegister,
    CompilationInfo,
    ExecutionInfo,
};

use crate::utils::Config;

static mut METADATA: Option<LanguageProviderMetadata> = None;
static METADATA_ONCE: Once = Once::new();

fn init_metadata() {
    METADATA_ONCE.call_once(|| {
        let mut metadata = LanguageProviderMetadata::new("rust", false);
        metadata.branches.push(LanguageBranch::new("rust", "1.38"));
        metadata.branches.push(LanguageBranch::new("rust", "1.39"));
        metadata.branches.push(LanguageBranch::new("rust", "1.40"));

        unsafe {
            METADATA = Some(metadata);
        }
    });
}

/// Rust language configuration.
#[derive(Clone, Debug, Deserialize)]
struct RustLanguageConfig {
    /// Path to the directory containing the Rust port of WaveTestLib.
    testlib_dir: PathBuf,
}

impl Config for RustLanguageConfig { }

/// Language provider of the Rust programming language.
struct RustLanguageProvider {
    /// Rust language configuration.
    config: RustLanguageConfig
}

impl RustLanguageProvider {
    /// Create a new `RustLanguageProvider` instance.
    fn new(config: RustLanguageConfig) -> Self {
        RustLanguageProvider { config }
    }
}

impl LanguageProvider for RustLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        let output_file = crate::utils::make_output_file_path(&program.file, output_dir);

        let mut ci = CompilationInfo::new("rustup", output_file);
        ci.compiler.args.push(String::from("run"));
        ci.compiler.args.push(program.language.version().to_owned());
        ci.compiler.args.push(String::from("rustc"));
        ci.compiler.args.push(String::from("-C"));
        ci.compiler.args.push(String::from("opt-level=2"));
        ci.compiler.args.push(String::from("--cfg"));
        ci.compiler.args.push(String::from("online_judge"));

        if kind.is_jury() {
            ci.compiler.args.push(String::from("-L"));
            ci.compiler.args.push(format!("{}", self.config.testlib_dir.display()));
        }

        ci.compiler.args.push(String::from("-o"));
        ci.compiler.args.push(format!("{}", ci.output_file.display()));

        ci.compiler.args.push(format!("{}", program.file.display()));

        Ok(ci)
    }

    fn execute(&self, program: &Program, _kind: ProgramKind)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        Ok(ExecutionInfo::new(&program.file))
    }
}

/// Name of the file containing Rust language configurations.
const RUST_CONFIG_FILE_NAME: &'static str = "config/rust-config.yaml";

/// Initialize language providers for the Rust programming language.
pub fn init_rust_providers(lang: &mut LanguageProviderRegister) -> Result<(), InitLanguageError> {
    init_metadata();

    let config = RustLanguageConfig::from_file(RUST_CONFIG_FILE_NAME)?;

    lang.register(Box::new(RustLanguageProvider::new(config)));

    Ok(())
}

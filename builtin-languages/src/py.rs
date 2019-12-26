//! This module defines the language provider for the Python programming language.
//!

use std::path::PathBuf;
use std::sync::Once;

use serde::Deserialize;

use crate::InitLanguageError;
use crate::utils::Config;

use judge::{
    Program,
    ProgramKind,
};
use judge::languages::{
    LanguageBranch,
    LanguageProvider,
    LanguageProviderMetadata,
    LanguageManager,
    CompilationInfo,
    ExecutionInfo,
};


static mut METADATA: Option<LanguageProviderMetadata> = None;
static METADATA_ONCE: Once = Once::new();

fn init_metadata() {
    METADATA_ONCE.call_once(|| {
        let mut metadata = LanguageProviderMetadata::new("python", true);
        metadata.branches.push(LanguageBranch::new("cpy", "3.6"));
        metadata.branches.push(LanguageBranch::new("cpy", "3.7"));
        metadata.branches.push(LanguageBranch::new("cpy", "3.8"));

        unsafe {
            METADATA = Some(metadata);
        }
    });
}

/// Provide configuration for python language providers.
#[derive(Debug, Clone, Deserialize)]
struct PythonLanguageConfig {
    testlib_module_dir: PathBuf,
}

impl Config for PythonLanguageConfig { }

/// Implement language provider for the Python programming language.
struct PythonLanguageProvider {
    /// Python language environment.
    config: PythonLanguageConfig,
}

impl PythonLanguageProvider {
    /// Create a new `PythonLanguageProvider` instance.
    fn new(config: PythonLanguageConfig) -> PythonLanguageProvider {
        PythonLanguageProvider { config }
    }
}

impl LanguageProvider for PythonLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { METADATA.as_ref().unwrap() }
    }

    fn compile(&self, _program: &Program, _kind: ProgramKind, _output_dir: Option<PathBuf>)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        // Because python is an interpreted language, this function is not reachable.
        unreachable!()
    }

    fn execute(&self, program: &Program, kind: ProgramKind)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        let mut ei = ExecutionInfo::new(format!("python{}", program.language.version()));
        ei.args.push(String::from("-OO"));
        ei.args.push(String::from("-B"));

        if kind.is_jury() {
            ei.envs.push((String::from("PYTHONPATH"),
                format!("{}", self.config.testlib_module_dir.display())));
        }

        ei.args.push(format!("\"{}\"", program.file.display()));
        Ok(ei)
    }
}

/// Name of the file containing python language provider configurations.
const PYTHON_LANG_CONFIG_FILE_NAME: &'static str = "py-config.yaml";

/// Initialize python language provider and related facilities.
pub fn init_py_providers() -> Result<(), InitLanguageError> {
    init_metadata();

    let lang_mgr = LanguageManager::singleton();
    let config = PythonLanguageConfig::from_file(PYTHON_LANG_CONFIG_FILE_NAME)?;
    lang_mgr.register(Box::new(PythonLanguageProvider::new(config)));

    Ok(())
}

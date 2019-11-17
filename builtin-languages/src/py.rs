use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Once;

use crate::InitLanguageError;

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
        let mut metadata = LanguageProviderMetadata::new(String::from("python"), true);
        metadata.branches.push(LanguageBranch::new("cpy", "3.6"));
        metadata.branches.push(LanguageBranch::new("cpy", "3.7"));
        metadata.branches.push(LanguageBranch::new("cpy", "3.8"));

        unsafe {
            METADATA = Some(metadata);
        }
    });
}

/// Name of the environment variable holding the path to the directory containing python module
/// file of `WaveTestLib`.
const WAVETESTLIB_DIR_ENV: &'static str = "WAVETESTLIB_PYMOD_DIR";

/// Provide environment related information for the python language provider.
struct PythonEnvironment {
    /// Path to the directory containing python module file of `WaveTestLib`.
    testlib_module_dir: PathBuf
}

impl PythonEnvironment {
    /// Create a new `PythonEnvironment` instance.
    fn new() -> Result<PythonEnvironment, InitLanguageError> {
        let testlib_module_dir = std::env::var(WAVETESTLIB_DIR_ENV)
            .map_err(|_| InitLanguageError::new(format!("Env variable \"{}\" not set.",
                WAVETESTLIB_DIR_ENV)))
            ?;

        Ok(PythonEnvironment {
            testlib_module_dir: PathBuf::from_str(&testlib_module_dir).unwrap()
        })
    }
}

/// Implement language provider for the Python programming language.
struct PythonLanguageProvider {
    /// Python language environment.
    env: PythonEnvironment
}

impl PythonLanguageProvider {
    /// Create a new `PythonLanguageProvider` instance.
    fn new(env: PythonEnvironment) -> PythonLanguageProvider {
        PythonLanguageProvider { env }
    }
}

impl LanguageProvider for PythonLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { METADATA.as_ref().unwrap() }
    }

    fn compile(&self, _program: &Program, _output_dir: Option<&Path>, _scheme: CompilationScheme)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        // Because python is an interpreted language, this function is not reachable.
        unreachable!()
    }

    fn execute(&self, program: &Program, scheme: ExecutionScheme)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        let mut ei = ExecutionInfo::new(
            &PathBuf::from(format!("python{}", program.language.version())));
        ei.args.push(String::from("-OO"));
        ei.args.push(String::from("-B"));

        match scheme {
            ExecutionScheme::Checker | ExecutionScheme::Interactor => {
                ei.envs.push((String::from("PYTHONPATH"),
                    format!("{}", self.env.testlib_module_dir.display())));
            },
            _ => ()
        };

        ei.args.push(format!("\"{}\"", program.file.display()));
        Ok(ei)
    }
}

/// Initialize python language provider and related facilities.
pub fn init_py_providers() -> Result<(), InitLanguageError> {
    init_metadata();

    let lang_mgr = LanguageManager::singleton();
    lang_mgr.register(Box::new(PythonLanguageProvider::new(PythonEnvironment::new()?)));

    Ok(())
}

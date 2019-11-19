//! This module provides definitions of C/C++ language providers.
//!


use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Once;

use judge::{
    Program,
    CompilationScheme,
};
use judge::languages::{
    LanguageBranch,
    LanguageProvider,
    LanguageProviderMetadata,
    LanguageManager,
    ExecutionScheme
};
use judge::engine::{
    CompilationInfo,
    ExecutionInfo
};

use crate::InitLanguageError;


static mut C_METADATA: Option<LanguageProviderMetadata> = None;
static mut CPP_METADATA: Option<LanguageProviderMetadata> = None;
static METADATA_ONCE: Once = Once::new();

fn init_metadata() {
    METADATA_ONCE.call_once(|| {
        let mut c_metadata = LanguageProviderMetadata::new("c", false);
        c_metadata.branches.push(LanguageBranch::new("clang", "c89"));
        c_metadata.branches.push(LanguageBranch::new("clang", "c95"));
        c_metadata.branches.push(LanguageBranch::new("clang", "c99"));
        c_metadata.branches.push(LanguageBranch::new("clang", "c11"));
        c_metadata.branches.push(LanguageBranch::new("clang", "c17"));
        c_metadata.branches.push(LanguageBranch::new("gnu", "c89"));
        c_metadata.branches.push(LanguageBranch::new("gnu", "c95"));
        c_metadata.branches.push(LanguageBranch::new("gnu", "c99"));
        c_metadata.branches.push(LanguageBranch::new("gnu", "c11"));
        c_metadata.branches.push(LanguageBranch::new("gnu", "c17"));
        unsafe {
            C_METADATA = Some(c_metadata);
        }

        let mut cpp_metadata = LanguageProviderMetadata::new("cpp", false);
        cpp_metadata.branches.push(LanguageBranch::new("clang", "c++98"));
        cpp_metadata.branches.push(LanguageBranch::new("clang", "c++03"));
        cpp_metadata.branches.push(LanguageBranch::new("clang", "c++11"));
        cpp_metadata.branches.push(LanguageBranch::new("clang", "c++14"));
        cpp_metadata.branches.push(LanguageBranch::new("clang", "c++17"));
        cpp_metadata.branches.push(LanguageBranch::new("gnu", "c++98"));
        cpp_metadata.branches.push(LanguageBranch::new("gnu", "c++03"));
        cpp_metadata.branches.push(LanguageBranch::new("gnu", "c++11"));
        cpp_metadata.branches.push(LanguageBranch::new("gnu", "c++14"));
        cpp_metadata.branches.push(LanguageBranch::new("gnu", "c++17"));
        unsafe {
            CPP_METADATA = Some(cpp_metadata);
        }
    });
}

const WAVETESTLIB_LIB_NAME: &'static str = "wavetest";

const WAVETESTLIB_INCLUDE_DIR_ENV: &'static str = "WAVETESTLIB_INCLUDE_DIR";
const WAVETESTLIB_LIB_DIR_ENV: &'static str = "WAVETESTLIB_LIB_DIR";

fn get_testlib_include_dir() -> Option<PathBuf> {
    std::env::var(WAVETESTLIB_INCLUDE_DIR_ENV).ok()
        .map(|v| PathBuf::from_str(&v).unwrap())
}

fn get_testlib_lib_dir() -> Option<PathBuf> {
    std::env::var(WAVETESTLIB_LIB_DIR_ENV).ok()
        .map(|v| PathBuf::from_str(&v).unwrap())
}

/// Provide environment related information to C/C++ language provider.
#[derive(Clone, Debug)]
struct CXXEnvironment {
    /// Path to the directory containing header files of wave test lib.
    testlib_include_dir: PathBuf,

    /// Path to the directory containing binary files of wave test lib.
    testlib_lib_dir: PathBuf
}

impl CXXEnvironment {
    /// Create a new `CXXEnvironment` whose information is collected from the current context.
    fn new() -> Result<CXXEnvironment, InitLanguageError> {
        let testlib_include_dir = get_testlib_include_dir()
            .ok_or_else(|| InitLanguageError::new(format!("Env variable \"{}\" not set.",
                WAVETESTLIB_INCLUDE_DIR_ENV)))?;
        let testlib_lib_dir = get_testlib_lib_dir()
            .ok_or_else(|| InitLanguageError::new(format!("Env variable \"{}\" not set.",
                WAVETESTLIB_LIB_DIR_ENV)))?;

        Ok(CXXEnvironment { testlib_include_dir, testlib_lib_dir })
    }
}

struct CXXLanguageProvider {
    env: CXXEnvironment
}

impl CXXLanguageProvider {
    fn new(env: CXXEnvironment) -> Self {
        CXXLanguageProvider { env }
    }

    fn compile(&self, program: &Program, output_dir: Option<PathBuf>, scheme: CompilationScheme)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        let compiler = match (program.language.language(), program.language.dialect()) {
            ("c", "gnu") => PathBuf::from("gcc"),
            ("cpp", "clang") => PathBuf::from("clang"),
            _ => panic!("unexpected language: ({}, {})",
                program.language.language(), program.language.dialect())
        };

        let output_file = crate::utils::make_output_file_path(&program.file, output_dir);

        let mut ci = CompilationInfo::new(compiler, output_file.clone());
        ci.compiler.args.push(String::from("-O2"));
        ci.compiler.args.push(format!("-std={}", program.language.version()));
        ci.compiler.args.push(String::from("-DONLINE_JUDGE"));

        match scheme {
            // Add waveteslib directory to include and library directories.
            CompilationScheme::Checker | CompilationScheme::Interactor => {
                ci.compiler.args.push(format!("-I\"{}\"", self.env.testlib_include_dir.display()));
                ci.compiler.args.push(format!("-L\"{}\"", self.env.testlib_lib_dir.display()));
            },
            _ => ()
        };

        ci.compiler.args.push(String::from("-o"));
        ci.compiler.args.push(format!("\"{}\"", output_file.display()));
        ci.compiler.args.push(format!("\"{}\"", program.file.display()));

        match scheme {
            // Push wavetestlib library to linker.
            CompilationScheme::Checker | CompilationScheme::Interactor => {
                ci.compiler.args.push(format!("-l{}", WAVETESTLIB_LIB_NAME));
            },
            _ => ()
        };

        Ok(ci)
    }

    fn execute(&self, program: &Program, _scheme: ExecutionScheme)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        Ok(ExecutionInfo::new(&program.file))
    }
}

/// Provide an implementation of the `LanguageProvider` trait for the C programming language.
struct CLanguageProvider {
    /// The common language provider designed for CXX.
    cxx_prov: CXXLanguageProvider
}

impl CLanguageProvider {
    /// Create a new `CLanguageProvider` instance.
    fn new(env: CXXEnvironment) -> Self {
        CLanguageProvider {
            cxx_prov: CXXLanguageProvider::new(env)
        }
    }
}

impl LanguageProvider for CLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { C_METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, output_dir: Option<PathBuf>, scheme: CompilationScheme)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.compile(program, output_dir, scheme)
    }

    fn execute(&self, program: &Program, scheme: ExecutionScheme)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.execute(program, scheme)
    }
}

/// Provide an implementation of the `LanguageProvider` trait for the C++ programming language.
struct CPPLanguageProvider {
    /// The common language provider designed for CXX.
    cxx_prov: CXXLanguageProvider
}

impl CPPLanguageProvider {
    /// Create a new `CPPLanguageProvider` instance.
    fn new(env: CXXEnvironment) -> Self {
        CPPLanguageProvider {
            cxx_prov: CXXLanguageProvider::new(env)
        }
    }
}

impl LanguageProvider for CPPLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { CPP_METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, output_dir: Option<PathBuf>, scheme: CompilationScheme)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.compile(program, output_dir, scheme)
    }

    fn execute(&self, program: &Program, scheme: ExecutionScheme)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.execute(program, scheme)
    }
}

pub fn init_cxx_providers() -> Result<(), InitLanguageError> {
    init_metadata();

    let env = CXXEnvironment::new()?;

    let lang_mgr = LanguageManager::singleton();
    lang_mgr.register(Box::new(CLanguageProvider::new(env.clone())));
    lang_mgr.register(Box::new(CPPLanguageProvider::new(env.clone())));

    Ok(())
}

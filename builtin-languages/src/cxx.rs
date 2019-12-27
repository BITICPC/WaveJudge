//! This module provides definitions of C/C++ language providers.
//!


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

use crate::InitLanguageError;
use crate::utils::Config;

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

/// Provide configuration for CXX language providers.
#[derive(Debug, Clone, Deserialize)]
struct CXXLanguageConfig {
    /// Path to the directory containing header files of WaveTestLib.
    testlib_include_dir: PathBuf,

    /// Path to the directory containing library files of WaveTestLib.
    testlib_lib_dir: PathBuf,
}

impl Config for CXXLanguageConfig { }

/// Name of the WaveTestLib library.
const WAVETESTLIB_LIB_NAME: &'static str = "wavetest";

struct CXXLanguageProvider {
    config: CXXLanguageConfig,
}

impl CXXLanguageProvider {
    fn new(config: CXXLanguageConfig) -> Self {
        CXXLanguageProvider { config }
    }

    fn compile(&self, program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        let compiler = match (program.language.language(), program.language.dialect()) {
            ("c", "gnu") => PathBuf::from("gcc"),
            ("c", "clang") => PathBuf::from("clang"),
            ("cpp", "gnu") => PathBuf::from("g++"),
            ("cpp", "clang") => PathBuf::from("clang++"),
            _ => panic!("unexpected language: ({}, {})",
                program.language.language(), program.language.dialect())
        };

        let output_file = crate::utils::make_output_file_path(&program.file, output_dir);

        let mut ci = CompilationInfo::new(compiler, output_file.clone());
        ci.compiler.args.push(String::from("-O2"));
        ci.compiler.args.push(format!("-std={}", program.language.version()));
        ci.compiler.args.push(String::from("-DONLINE_JUDGE"));

        if kind.is_jury() {
            ci.compiler.args.push(
                format!("-I{}", self.config.testlib_include_dir.display()));
            ci.compiler.args.push(format!("-L{}", self.config.testlib_lib_dir.display()));
        }

        ci.compiler.args.push(String::from("-o"));
        ci.compiler.args.push(format!("{}", output_file.display()));
        ci.compiler.args.push(format!("{}", program.file.display()));

        if kind.is_jury() {
            ci.compiler.args.push(format!("-l{}", WAVETESTLIB_LIB_NAME));
        }

        Ok(ci)
    }

    fn execute(&self, program: &Program, _kind: ProgramKind)
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
    fn new(config: CXXLanguageConfig) -> Self {
        CLanguageProvider {
            cxx_prov: CXXLanguageProvider::new(config)
        }
    }
}

impl LanguageProvider for CLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { C_METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.compile(program, kind, output_dir)
    }

    fn execute(&self, program: &Program, kind: ProgramKind)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.execute(program, kind)
    }
}

/// Provide an implementation of the `LanguageProvider` trait for the C++ programming language.
struct CPPLanguageProvider {
    /// The common language provider designed for CXX.
    cxx_prov: CXXLanguageProvider
}

impl CPPLanguageProvider {
    /// Create a new `CPPLanguageProvider` instance.
    fn new(config: CXXLanguageConfig) -> Self {
        CPPLanguageProvider {
            cxx_prov: CXXLanguageProvider::new(config)
        }
    }
}

impl LanguageProvider for CPPLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { CPP_METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.compile(program, kind, output_dir)
    }

    fn execute(&self, program: &Program, kind: ProgramKind)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        self.cxx_prov.execute(program, kind)
    }
}

/// Name of the file containing CXX language configurations.
const CXX_LANG_CONFIG_FILE_NAME: &'static str = "cpp-config.yaml";

pub fn init_cxx_providers(lang: &mut LanguageProviderRegister) -> Result<(), InitLanguageError> {
    init_metadata();

    let config = CXXLanguageConfig::from_file(CXX_LANG_CONFIG_FILE_NAME)?;

    lang.register(Box::new(CLanguageProvider::new(config.clone())));
    lang.register(Box::new(CPPLanguageProvider::new(config.clone())));

    Ok(())
}

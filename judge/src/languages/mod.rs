//! This module implements language related facilities used in the judge.
//!

mod loader;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use libloading::Library;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use sandbox::SystemCall;

use super::{Program, ProgramKind};

pub use loader::{
    Error as LoadDylibError,
    ErrorKind as LoadDylibErrorKind,
};

/// Identifier of a programming language and its runtime environment.
///
/// Language identifiers is a 3-tuple (language, dialect, version) that uniquely identifies a
/// programming language and its runtime environment. Language providers can be filtered out by the
/// `language` part, and `dialect` and `version` part will be sent to the language provider to
/// determine and initialize corresponding environment when something needs to be executed.
///
/// The last 2 fields of a language identifier, (dialect, version) is called the language's branch
/// which can be represented using the `LanguageBranch` structure.
///
/// For example, suppose we have a language identifier (`cpp`, `clang`, `11`). The C++ language
/// provider will be selected by this language identifier, and the language provider will choose to
/// use `clang` compiler toolchains to compile source code with C++11 features available.
#[derive(Clone, Debug, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LanguageIdentifier(String, LanguageBranch);

impl LanguageIdentifier {
    /// Create a new `LanguageIdentifier` instance.
    pub fn new<T>(language: T, branch: LanguageBranch) -> Self
        where T: Into<String> {
        LanguageIdentifier(language.into(), branch)
    }

    /// Get the language part of the identifier.
    pub fn language(&self) -> &str {
        &self.0
    }

    /// Get the branch of the language.
    pub fn branch(&self) -> &LanguageBranch {
        &self.1
    }

    /// Get the dialect part of the identifier.
    pub fn dialect(&self) -> &str {
        self.branch().dialect()
    }

    /// Get the version part of the identifier.
    pub fn version(&self) -> &str {
        self.branch().version()
    }
}

impl PartialEq for LanguageIdentifier {
    fn eq(&self, other: &LanguageIdentifier) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Display for LanguageIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("({}, {}, {})", self.language(), self.dialect(), self.version()))
    }
}

/// Represent a branch of a language.
///
/// A branch of a language is a 2-tuple (String, String) whose first field represents the dialect of
/// the language and second field represents the version of the language.
#[derive(Clone, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LanguageBranch(String, String);

impl LanguageBranch {
    /// Create a new `LanguageBranch` instance.
    pub fn new<T1, T2>(dialect: T1, version: T2) -> Self
        where T1: Into<String>, T2: Into<String> {
        LanguageBranch(dialect.into(), version.into())
    }

    /// Get the dialect of the branch.
    pub fn dialect(&self) -> &str {
        &self.0
    }

    /// Get the version of the branch.
    pub fn version(&self) -> &str {
        &self.1
    }
}

impl PartialEq for LanguageBranch {
    fn eq(&self, other: &LanguageBranch) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Display for LanguageBranch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("({}, {})", self.dialect(), self.version()))
    }
}

/// Provide metadata about a language provider.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LanguageProviderMetadata {
    /// The name of the language. This field corresponds to the first field of a
    /// `LanguageIdentifier`.
    pub name: String,

    /// All supported branches by this language provider.
    pub branches: Vec<LanguageBranch>,

    /// Does the programs written in this language need to be compiled into some form (binary code,
    /// bytecode, etc.) by some compiler program before it can be executed?
    pub interpreted: bool
}

impl LanguageProviderMetadata {
    /// Create a new `LanguageProviderMetadata` instance.
    ///
    /// `name` represents the name of the language, which corresponds to the first field of a
    /// `LanguageIdentifier` value. `interpreted` indicates whether programs written in this
    /// language is interpreted, and does not need to be compiled into some form (binary code,
    /// bytecode, etc.) before they can be executed.
    pub fn new<T>(name: T, interpreted: bool) -> Self
        where T: Into<String> {
        LanguageProviderMetadata {
            name: name.into(),
            branches: Vec::new(),
            interpreted
        }
    }
}

/// This trait defines functions to be implemented by language providers who provides the ability to
/// compile and execute a program written in some language. This trait is object safe and is
/// commonly used in trait objects.
///
/// Implementors of this trait should be thread safe since this trait forces the `Sync` trait.
pub trait LanguageProvider : Sync {
    /// Get metadata about the language provider. The returned metadata should be statically
    /// allocated and has the `'static` lifetime specifier.
    fn metadata(&self) -> &'static LanguageProviderMetadata;

    /// Create a `CompilationInfo` instance containing necessary information used to compile the
    /// source code.
    fn compile(&self, program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> std::result::Result<CompilationInfo, Box<dyn std::error::Error>>;

    /// Create an `ExecutionInfo` instance containing necessary information used to execute the
    /// program.
    fn execute(&self, program: &Program, kind: ProgramKind)
        -> std::result::Result<ExecutionInfo, Box<dyn std::error::Error>>;
}

/// Provide thread-unsafe implementation for `LanguageManager`.
struct LanguageManagerImpl {
    /// All loaded libraries.
    libs: Vec<Library>,

    /// All registered providers.
    providers: HashMap<String, Vec<Arc<Box<dyn LanguageProvider>>>>,
}

impl LanguageManagerImpl {
    /// Create a new `LanguageManagerImpl` object.
    fn new() -> Self {
        LanguageManagerImpl {
            libs: Vec::new(),
            providers: HashMap::new(),
        }
    }

    /// Register a language provider in the language manager.
    fn register(&mut self, lang_prov: Box<dyn LanguageProvider>) {
        let metadata = lang_prov.metadata();
        if let Some(ref mut prov) = self.providers.get_mut(&metadata.name) {
            prov.push(Arc::new(lang_prov));
        } else {
            self.providers.insert(metadata.name.clone(), vec![Arc::new(lang_prov)]);
        }

        log::info!("Language provider for language \"{}\" registered.", metadata.name);
    }

    /// Find a `LanguageProvider` instance registered in this `LanguageManager` that is capable of
    /// handling the given language environment.
    ///
    /// If none of the `LanguageProviders` registered in this instance is suitable, then returns
    /// `None`.
    fn find(&self, lang: &LanguageIdentifier) -> Option<Arc<Box<dyn LanguageProvider>>> {
        if let Some(prov) = self.providers.get(lang.language()) {
            for provider in prov {
                let metadata = provider.metadata();
                if metadata.branches.contains(lang.branch()) {
                    return Some(provider.clone());
                }
            }
        }

        None
    }

    /// Get all registered languages inside this language manager.
    fn languages(&self) -> Vec<LanguageIdentifier> {
        let mut lang = Vec::new();
        for (name, prov) in &self.providers {
            for provider in prov {
                let metadata = provider.metadata();
                for branch in &metadata.branches {
                    lang.push(LanguageIdentifier::new(name.clone(), branch.clone()));
                }
            }
        }

        lang
    }
}

impl Drop for LanguageManagerImpl {
    fn drop(&mut self) {
        // The order of dropping is critical since the libraries must strictly outlive the language
        // providers and Rust cannot guarantee this.

        // Drop all language providers first.
        self.providers.clear();

        // Then drop all the loaded libraries.
        self.libs.clear();
    }
}

/// Provide centralized language management services. This structure and its related facilities are
/// thread safe.
pub struct LanguageManager {
    imp: RwLock<LanguageManagerImpl>,
}

impl LanguageManager {
    /// Create a new `LanguageManager` instance.
    pub fn new() -> Self {
        LanguageManager {
            imp: RwLock::new(LanguageManagerImpl::new()),
        }
    }

    /// Load the specifid dynamic library that contains language providers.
    pub fn load_dylib<P>(&self, file: &P) -> Result<(), LoadDylibError>
        where P: ?Sized + AsRef<Path> {
        let mut lock = self.imp.write().unwrap();
        let mut register = LanguageProviderRegister::new(&mut *lock);
        let lib = loader::load(file, &mut register)?;
        lock.libs.push(lib);

        Ok(())
    }

    /// Register a language provider in the language manager.
    pub fn register(&self, lang_prov: Box<dyn LanguageProvider>) {
        let mut lock = self.imp.write().unwrap();
        lock.register(lang_prov);
    }

    /// Find a `LanguageProvider` instance registered in this `LanguageManager` that is capable of
    /// handling the given language environment.
    ///
    /// If none of the `LanguageProviders` registered in this instance is suitable, then returns
    /// `None`.
    pub fn find(&self, lang: &LanguageIdentifier) -> Option<Arc<Box<dyn LanguageProvider>>> {
        let lock = self.imp.read().unwrap();
        lock.find(lang)
    }

    /// Get all registered languages inside this language manager.
    pub fn languages(&self) -> Vec<LanguageIdentifier> {
        let lock = self.imp.read().unwrap();
        lock.languages()
    }
}

/// Provide a register for language providers to register themselves into the language manager.
pub struct LanguageProviderRegister<'a> {
    /// The underlying thread unsafe implementation of a language manager.
    lang: &'a mut LanguageManagerImpl,
}

impl<'a> LanguageProviderRegister<'a> {
    /// Create a new `LanguageProviderRegister` object.
    fn new(lang: &'a mut LanguageManagerImpl) -> Self {
        LanguageProviderRegister { lang }
    }

    /// Register the given language provider in the language manager.
    pub fn register(&mut self, lang_prov: Box<dyn LanguageProvider>) {
        self.lang.register(lang_prov);
    }
}

/// Provide necessary information to execute a program.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExecutionInfo {
    /// Path to the executable file to be executed.
    pub executable: PathBuf,

    /// Arguments to be passed to the program.
    pub args: Vec<String>,

    /// Environment variables to be passed to the program.
    pub envs: Vec<(String, String)>,

    /// System call whitelist specified for this execution.
    pub syscall_whitelist: Vec<SystemCall>,
}

impl ExecutionInfo {
    /// Create a new `ExecutionInfo` instance.
    pub fn new<T>(executable: T) -> ExecutionInfo
        where T: Into<PathBuf> {
        ExecutionInfo {
            executable: executable.into(),
            args: Vec::new(),
            envs: Vec::new(),
            syscall_whitelist: Vec::new(),
        }
    }
}

/// Provide necessary information to compile a source program.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CompilationInfo {
    /// Information necessary to execute the compiler instance.
    pub compiler: ExecutionInfo,

    /// Path to the output file generated by the compiler. These files will be sent to the language
    /// provider creating this `CompilerInfo` instance to execute the program.
    pub output_file: PathBuf
}

impl CompilationInfo {
    /// Create a new `CompilationInfo` instance.
    pub fn new<T1, T2>(compiler: T1, output_file: T2) -> CompilationInfo
        where T1: Into<PathBuf>, T2: Into<PathBuf> {
        CompilationInfo {
            compiler: ExecutionInfo::new(compiler),
            output_file: output_file.into()
        }
    }
}

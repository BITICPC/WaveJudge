//! This module implements language related facilities used in the judge.
//!

pub mod loader;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::{Arc, Once, RwLock};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use super::{Program, CompilationScheme};
use super::engine::{CompilationInfo, ExecutionInfo};


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
#[derive(Clone, Debug)]
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
#[derive(Clone, Eq, Debug)]
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
    fn compile(&self, program: &Program, output_dir: Option<PathBuf>, scheme: CompilationScheme)
        -> std::result::Result<CompilationInfo, Box<dyn std::error::Error>>;

    /// Create an `ExecutionInfo` instance containing necessary information used to execute the
    /// program.
    fn execute(&self, program: &Program, scheme: ExecutionScheme)
        -> std::result::Result<ExecutionInfo, Box<dyn std::error::Error>>;
}

/// Represent scheme of an execution.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExecutionScheme {
    /// The program to be executed is a judgee.
    Judgee,

    /// The program to be executed is a checker.
    Checker,

    /// The program to be executed is an interactor.
    Interactor
}

/// Provide centralized language management services. This structure and its related facilities are
/// thread safe.
pub struct LanguageManager {
    providers: RwLock<HashMap<String, Vec<Arc<Box<dyn LanguageProvider>>>>>
}

/// This global static mutable variable stores an atomic reference to the singleton
/// `LanguageManager` instance, and `LANG_MANAGER_ONCE` is the `Once` guard used to initialize it.
static mut LANG_MANAGER: Option<Arc<LanguageManager>> = None;
static LANG_MANAGER_ONCE: Once = Once::new();

impl LanguageManager {
    /// Create a new `LanguageManager` instance.
    fn new() -> LanguageManager {
        LanguageManager {
            providers: RwLock::new(HashMap::new())
        }
    }

    /// Get the singleton instance of `LanguageManager` in the application's global scope.
    pub fn singleton() -> Arc<LanguageManager> {
        LANG_MANAGER_ONCE.call_once(|| {
            trace!("Initializing language manager");
            unsafe {
                LANG_MANAGER = Some(Arc::new(LanguageManager::new()));
            }
        });

        unsafe {
            LANG_MANAGER.as_ref().unwrap()
        }.clone()
    }

    /// Register a language provider in the language manager.
    pub fn register(&self, lang_prov: Box<dyn LanguageProvider>) {
        let metadata = lang_prov.metadata();
        let mut lock = self.providers.write().unwrap();
        if let Some(ref mut prov) = lock.get_mut(&metadata.name) {
            prov.push(Arc::new(lang_prov));
        } else {
            lock.insert(metadata.name.clone(), vec![Arc::new(lang_prov)]);
        }

        info!("Language provider for language \"{}\" registered.", metadata.name);
    }

    /// Find a `LanguageProvider` instance registered in this `LanguageManager` that is capable of
    /// handling the given language environment.
    ///
    /// If none of the `LanguageProviders` registered in this instance is suitable, then returns
    /// `None`.
    pub fn find(&self, lang: &LanguageIdentifier) -> Option<Arc<Box<dyn LanguageProvider>>> {
        let lock = self.providers.read().unwrap();
        if let Some(prov) = lock.get(lang.language()) {
            for provider in prov {
                let metadata = provider.metadata();
                if metadata.branches.contains(lang.branch()) {
                    return Some(provider.clone());
                }
            }
        }

        None
    }
}

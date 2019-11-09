//! This module implements language related facilities used in the judge.
//!

pub mod loader;

use std::collections::HashMap;
use std::sync::{Arc, Once, RwLock};

use super::Program;
use super::engine::{CompilationInfo, CompilationScheme, ExecutionInfo};


/// Identifier of a programming language and its runtime environment.
///
/// Language identifiers is a 3-tuple (language, dialect, version) that uniquely
/// identifies a programming language and its runtime environment. Language
/// providers can be filtered out by the `language` part, and `dialect` and
/// `version` part will be sent to the language provider to determine and
/// initialize corresponding environment when something needs to be executed.
///
/// The last 2 fields of a language identifier, (dialect, version) is called the
/// language's branch which can be represented using the `LanguageBranch`
/// structure.
///
/// For example, suppose we have a language identifier (`cpp`, `clang`, `11`).
/// The C++ language provider will be selected by this language identifier,
/// and the language provider will choose to use `clang` compiler toolchains
/// to compile source code with C++11 features available.
#[derive(Clone)]
pub struct LanguageIdentifier(String, LanguageBranch);

impl LanguageIdentifier {
    /// Create a new `LanguageIdentifier` instance.
    pub fn new(language: &str, branch: LanguageBranch) -> LanguageIdentifier {
        LanguageIdentifier(language.to_owned(), branch)
    }

    /// Get the language part of the identifier.
    pub fn language(&self) -> &str {
        &self.0
    }

    /// Get the dialect part of the identifier.
    pub fn dialect(&self) -> &str {
        self.1.dialect()
    }

    /// Get the version part of the identifier.
    pub fn version(&self) -> &str {
        self.1.version()
    }
}

impl PartialEq for LanguageIdentifier {
    fn eq(&self, other: &LanguageIdentifier) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

/// Represent a branch of a language.
///
/// A branch of a language is a 2-tuple (String, String) whose first field
/// represents the dialect of the language and second field represents the
/// version of the language.
#[derive(Clone, Eq)]
pub struct LanguageBranch(String, String);

impl LanguageBranch {
    /// Create a new `LanguageBranch` instance.
    pub fn new(dialect: &str, version: &str) -> LanguageBranch {
        LanguageBranch(dialect.to_owned(), version.to_owned())
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

/// Provide metadata about a language provider.
pub struct LanguageProviderMetadata {
    /// The name of the language. This field corresponds to the first field of
    /// a `LanguageIdentifier`.
    pub name: String,

    /// All supported branches by this language provider.
    pub branches: Vec<LanguageBranch>,

    /// Does the programs written in this language need to be compiled into some
    /// form (binary code, bytecode, etc.) by some compiler program before it
    /// can be executed?
    pub interpreted: bool
}

impl LanguageProviderMetadata {
    /// Create a new `LanguageProviderMetadata` instance.
    ///
    /// `name` represents the name of the language, which corresponds to the
    /// first field of a `LanguageIdentifier` value. `interpreted` indicates
    /// whether programs written in this language is interpreted, and does not
    /// need to be compiled into some form (binary code, bytecode, etc.) before
    /// they can be executed.
    pub fn new(name: String, interpreted: bool) -> LanguageProviderMetadata {
        LanguageProviderMetadata {
            name,
            branches: Vec::new(),
            interpreted
        }
    }
}

/// This trait defines functions to be implemented by language providers who
/// provides the ability to compile and execute a program written in some
/// language. This trait is object safe and is commonly used in trait objects.
///
/// Implementors of this trait should be thread safe since this trait forces
/// the `Sync` trait.
pub trait LanguageProvider : Sync {
    /// Get metadata about the language provider. The returned metadata should
    /// be statically allocated and has the `'static` lifetime specifier.
    fn metadata(&self) -> &'static LanguageProviderMetadata;

    /// Create a `CompilationInfo` instance containing necessary information
    /// used to compile the source code.
    ///
    /// It is guaranteed that `program.format` is `ProgramFormat::Source`.
    fn compile(&self, program: &Program, scheme: CompilationScheme)
        -> std::result::Result<CompilationInfo, Box<dyn std::error::Error>>;

    /// Create an `ExecutionInfo` instance containing necessary information used
    /// to execute the program.
    ///
    /// If this language is an interpreted language, then it is guaranteed that
    /// `program.format` is `ProgramFormat::Source`; otherwise it is guaranteed
    /// that `program.format` is `ProgramFormat::Executable` and `program.file`
    /// is the output file generated by the compiler returned by an early call
    /// to the `compile` function of this language provider.
    fn execute(&self, program: &Program, branch: &LanguageBranch)
        -> std::result::Result<ExecutionInfo, Box<dyn std::error::Error>>;
}

/// Provide centralized language management services. This structure and its
/// related facilities are thread safe.
pub struct LanguageManager {
    providers: RwLock<HashMap<String, Vec<Box<dyn LanguageProvider>>>>
}

/// This global static mutable variable stores an atomic reference to the
/// singleton `LanguageManager` instance, and `LANG_MANAGER_ONCE` is the `Once`
/// guard used to initialize it.
static mut LANG_MANAGER: Option<Arc<LanguageManager>> = None;
static LANG_MANAGER_ONCE: Once = Once::new();

impl LanguageManager {
    /// Create a new `LanguageManager` instance.
    fn new() -> LanguageManager {
        LanguageManager {
            providers: RwLock::new(HashMap::new())
        }
    }

    /// Get the singleton instance of `LanguageManager` in the application's
    /// global scope.
    pub fn singleton() -> Arc<LanguageManager> {
        LANG_MANAGER_ONCE.call_once(|| {
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
            prov.push(lang_prov);
        } else {
            lock.insert(metadata.name.clone(), vec![lang_prov]);
        }
    }
}

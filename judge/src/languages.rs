//! This module implements language related facilities used in the judge.
//!

/// Identifier of a programming language and its runtime environment.
///
/// Language identifiers is a 3-tuple (language, dialect, version) that uniquely
/// identifies a programming language and its runtime environment. Language
/// providers can be filtered out by the `language` part, and `dialect` and
/// `version` part will be sent to the language provider to determine and
/// initialize corresponding environment when something needs to be executed.
///
/// For example, suppose we have a language identifier (`cpp`, `clang`, `11`).
/// The C++ language provider will be selected by this language identifier,
/// and the language provider will choose to use `clang` compiler toolchains
/// to compile source code with C++11 features available.
#[derive(Clone)]
pub struct LanguageIdentifier(String, String, String);

impl LanguageIdentifier {
    /// Create a new `LanguageIdentifier` instance.
    pub fn new(language: &str, dialect: &str, version: &str)
        -> LanguageIdentifier {
        LanguageIdentifier(
            language.to_owned(), dialect.to_owned(), version.to_owned())
    }

    /// Get the language part of the identifier.
    pub fn language(&self) -> &str {
        &self.0
    }

    /// Get the dialect part of the identifier.
    pub fn dialect(&self) -> &str {
        &self.1
    }

    /// Get the version part of the identifier.
    pub fn version(&self) -> &str {
        &self.2
    }
}

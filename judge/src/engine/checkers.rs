//! This module implements built-in answer checkers used in standard judge mode.
//!

use std::fs::File;

use crate::BuiltinCheckers;
use super::io::{TokenizedRead, TokenizedReader};


/// Provide a trait for checker factories. This trait is object safe.
pub trait CheckerFactory {
    /// Create a checker instance.
    fn create(&self) -> Box<dyn Checker>;
}

/// Provide a trait for built-in answer checkers.
pub trait Checker {
    /// Perform check strategy on the given check context.
    fn check(&self, context: &mut CheckerContext) -> std::io::Result<CheckerResult>;
}

/// Provide context information for checkers.
pub struct CheckerContext {
    /// Input file of the test case.
    pub input: TokenizedReader<File>,

    /// Answer file of the test case.
    pub answer: TokenizedReader<File>,

    /// Judgee's output file.
    pub user_output: TokenizedReader<File>
}

impl CheckerContext {
    /// Create a new `CheckerContext` instance.
    pub fn new(
        input: TokenizedReader<File>,
        answer: TokenizedReader<File>,
        user_output: TokenizedReader<File>) -> CheckerContext {
        CheckerContext {
            input,
            answer,
            user_output
        }
    }
}

/// Represent the result of a checker.
pub struct CheckerResult {
    /// Can the answer gave by the judgee be accepted?
    pub accepted: bool,

    /// Comment by the checker, if any.
    pub comment: Option<String>
}

impl CheckerResult {
    /// Create a new `CheckerResult` instance representing an accepted result.
    pub fn accepted(comment: Option<String>) -> CheckerResult {
        CheckerResult {
            accepted: true,
            comment
        }
    }

    /// Create a new `CheckerResult` instance representing a rejected result.
    pub fn rejected(comment: Option<String>) -> CheckerResult {
        CheckerResult {
            accepted: false,
            comment
        }
    }
}

/// Factory implementation for the default built-in checker.
pub struct DefaultCheckerFactory;

impl DefaultCheckerFactory {
    /// Create a new `DefaultCheckerFactory` instance.
    pub fn new() -> DefaultCheckerFactory {
        DefaultCheckerFactory { }
    }
}

impl CheckerFactory for DefaultCheckerFactory {
    fn create(&self) -> Box<dyn Checker> {
        Box::new(DefaultChecker::new())
    }
}

/// The default checker implementation. This built-in checker implementation corresponds to the
/// `BuiltinCheckers::Default` variant.
pub struct DefaultChecker;

impl DefaultChecker {
    /// Create a new `DefaultChecker` instance.
    pub fn new() -> DefaultChecker {
        DefaultChecker { }
    }
}

impl Checker for DefaultChecker {
    fn check(&self, context: &mut CheckerContext) -> std::io::Result<CheckerResult> {
        let mut token_counter = 0;

        while let Some(expected_token) = context.answer.read_token()? {
            let user_token = match context.user_output.read_token()? {
                Some(t) => t,
                None => return Ok(CheckerResult::rejected(
                    Some(format!("expect \"{}\", but found EOF", expected_token))))
            };
            if expected_token != user_token {
                return Ok(CheckerResult::rejected(
                    Some(format!("expect \"{}\", but found \"{}\"", expected_token, user_token))));
            }

            token_counter += 1;
        }

        // Check if we can hit EOF on the user's output stream.
        if let Some(user_token) = context.user_output.read_token()? {
            return Ok(CheckerResult::rejected(
                Some(format!("expect EOF, but found \"{}\"", user_token))));
        }

        Ok(CheckerResult::accepted(Some(format!("OK: {} tokens.", token_counter))))
    }
}

/// Get the corresponding built-in checker factory instance specified by the `BuiltinCheckers` enum.
pub fn get_checker_factory(checker: BuiltinCheckers) -> Box<dyn CheckerFactory> {
    match checker {
        BuiltinCheckers::Default => Box::new(DefaultCheckerFactory::new())
    }
}

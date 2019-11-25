//! This module implements built-in answer checkers used in standard judge mode.
//!

use std::fs::File;
use std::str::FromStr;

use crate::BuiltinCheckers;
use super::io::{TokenizedRead, TokenizedReader};


/// Type prototype for a built-in answer checker.
pub type Checker = fn(&mut CheckerContext) -> std::io::Result<CheckerResult>;

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
#[derive(Debug)]
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

/// A boilerplate function that executes some common logic of all built-in checkers, such as
/// synchronizing tokens from both stream readers given to the checker. The concrete answer
/// checking logic that determines whether two tokens are the same answer is given as a `Fn` value.
fn builtin_checker_exec<C>(context: &mut CheckerContext, token_checker: C)
    -> std::io::Result<CheckerResult>
    where C: Fn(&str, &str) -> (bool, Option<String>) {
    let mut token_counter = 0;

    while let Some(expected_token) = context.answer.read_token()? {
        let user_token = match context.user_output.read_token()? {
            Some(t) => t,
            None => return Ok(CheckerResult::rejected(
                Some(format!("expect \"{}\", but found EOF", expected_token))))
        };

        let (accepted, comment) = token_checker(&expected_token, &user_token);
        if !accepted {
            return Ok(CheckerResult::rejected(comment));
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

/// This function implements the default checker's logic.
fn default_checker(context: &mut CheckerContext) -> std::io::Result<CheckerResult> {
    builtin_checker_exec(context, |expected_token, user_token| {
        if expected_token != user_token {
            (false, Some(format!("expected \"{}\", but found \"{}\".",
                expected_token, user_token)))
        } else {
            (true, None)
        }
    })
}

/// This function implements the floating point aware checker's logic.
fn floating_point_aware_checker(context: &mut CheckerContext) -> std::io::Result<CheckerResult> {
    builtin_checker_exec(context, |expected_token, user_token| {
        fn get_error_msg(expected_token: &str, user_token: &str, error: f64) -> String {
            format!("expected: \"{}\", but found: \"{}\", error is {}.",
                expected_token, user_token, error)
        }

        if expected_token == user_token {
            (true, None)
        } else {
            let expected_fp = match f64::from_str(expected_token) {
                Ok(fp) => fp,
                Err(..) => return (false, Some(
                    get_error_msg(expected_token, user_token, std::f64::NAN)))
            };
            let user_fp = match f64::from_str(user_token) {
                Ok(fp) => fp,
                Err(..) => return (false, Some(
                    get_error_msg(expected_token, user_token, std::f64::NAN)))
            };

            match (expected_fp.is_nan(), user_fp.is_nan()) {
                (true, true) => return (true, None),
                (false, true) | (true, false) =>
                    return (false, Some(
                        get_error_msg(expected_token, user_token, std::f64::NAN))),
                (false, false) => ()
            };

            let fp_abs_error = (user_fp - expected_fp).abs();
            let fp_rel_error = ((user_fp - expected_fp) / expected_fp).abs();
            let fp_error = if fp_abs_error < fp_rel_error {
                fp_abs_error
            } else {
                fp_rel_error
            };

            const TOLERANCE: f64 = 1e-6;
            if fp_error > TOLERANCE {
                (false, Some(get_error_msg(expected_token, user_token, fp_error)))
            } else {
                (true, None)
            }
        }
    })
}

/// This function implements the case insensitive checker's logic.
fn case_insensitive_checker(context: &mut CheckerContext) -> std::io::Result<CheckerResult> {
    builtin_checker_exec(context, |expected_token, user_token| {
        if expected_token.eq_ignore_ascii_case(user_token) {
            (true, None)
        } else {
            (false, Some(format!("expected \"{}\", found \"{}\"", expected_token, user_token)))
        }
    })
}

/// Get the corresponding built-in checker specified by the `BuiltinCheckers` enum.
pub fn get_checker(checker: BuiltinCheckers) -> Checker {
    match checker {
        BuiltinCheckers::Default => default_checker,
        BuiltinCheckers::FloatingPointAware => floating_point_aware_checker,
        BuiltinCheckers::CaseInsensitive => case_insensitive_checker
    }
}

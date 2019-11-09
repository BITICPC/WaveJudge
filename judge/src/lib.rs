//! This crate implements the core logic of the judge.
//!

#[macro_use]
extern crate error_chain;
extern crate log;
extern crate libc;
extern crate nix;
extern crate sandbox;
extern crate libloading;

pub mod engine;
pub mod languages;

use std::ops::{BitAnd, BitAndAssign};
use std::path::PathBuf;
use std::time::Duration;

use sandbox::{MemorySize, ProcessResourceUsage, ProcessExitStatus};

use languages::LanguageIdentifier;


error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }
}


/// Describe a judge task.
#[derive(Clone)]
pub struct JudgeTaskDescriptor {
    /// Program to be judged (called the judgee).
    pub program: Program,

    /// Judge mode.
    pub mode: JudgeMode,

    /// Resource limits.
    pub limits: ResourceLimits,

    /// The test suite, consisting of multiple test cases described by a 2-tuple
    /// (input_file, output_file).
    pub test_suite: Vec<TestCaseDescriptor>,
}

impl JudgeTaskDescriptor {
    /// Create a new `JudgeTaskDescriptor` instance.
    pub fn new(program: Program) -> JudgeTaskDescriptor {
        JudgeTaskDescriptor {
            program,
            mode: JudgeMode::default(),
            limits: ResourceLimits::default(),
            test_suite: Vec::new()
        }
    }
}

/// Represent a program stored in local disk file, along with the corresponding
/// language environment. The program file may either be a source file or an
/// executable file.
#[derive(Clone)]
pub struct Program {
    /// Path to the program file.
    pub file: PathBuf,

    /// Language and corresponding branch in which the program is written.
    pub language: LanguageIdentifier,

    /// Format of the program.
    pub format: ProgramFormat
}

/// Represent the format of a program given in a `Program` value.
#[derive(Clone, Copy)]
pub enum ProgramFormat {
    /// The program is a source program.
    Source,

    // The program is in some executable form, e.g. machine code, bytecode, etc.
    Executable
}

/// Resource limits that should be applied to the judgee when executing judge.
#[derive(Clone, Copy)]
pub struct ResourceLimits {
    /// CPU time limit.
    pub cpu_time_limit: Duration,

    /// Real time limit.
    pub real_time_limit: Duration,

    /// Memory limit.
    pub memory_limit: MemorySize,
}

impl Default for ResourceLimits {
    fn default() -> ResourceLimits {
        ResourceLimits {
            cpu_time_limit: Duration::from_secs(1),
            real_time_limit: Duration::from_secs(3),
            memory_limit: MemorySize::MegaBytes(256)
        }
    }
}

/// The judge mode.
#[derive(Clone)]
pub enum JudgeMode {
    /// Standard judge mode. The input of the judgee is redirected to the input
    /// file of each test case, and the output of the judgee is compared
    /// against the answer file of corresponding test case token by token
    /// literally. Semantics of tokens (e.g. floating point numbers) will not
    /// be automatically analyzed under standard mode.
    Standard,

    /// Special judge mode. The input of the judgee is redirected to the input
    /// file of each test case, and the output of the judgee, together with
    /// the input and answer of the test case, are sent to a user provided
    /// program given in the variant field who is responsible for checking
    /// the correctness of the answer.
    SpecialJudge(Program),

    /// Interactive mode. The input and output of the judgee is piped from / to
    /// a user provided program called the interactor. The input and answer of
    /// the test case is sent into the interactor, too. The interator is
    /// responsible for checking the correctness of the behavior of the judgee.
    Interactive(Program)
}

impl Default for JudgeMode {
    fn default() -> JudgeMode {
        JudgeMode::Standard
    }
}

/// Describe a test case.
#[derive(Clone)]
pub struct TestCaseDescriptor {
    /// Path to the input file.
    pub input_file: PathBuf,

    /// Path to the output file.
    pub output_file: PathBuf
}

/// Result of a judge task.
pub struct JudgeResult {
    /// Overall verdict of the judge task.
    pub verdict: Verdict,

    /// Overall resource usage statistics.
    pub rusage: ProcessResourceUsage,

    /// Judge results of every executed test cases in the test suite.
    test_suite: Vec<TestCaseResult>
}

impl JudgeResult {
    /// Get judge results of every executed test cases in the test suite. The
    /// order of the `TestCaseResult` instances in the returned slice is the
    /// same as the order `TestCaseDescriptor` instances was added to the judge
    /// task descriptor.
    ///
    /// It should be noticed that the length of the returned slice could be
    /// smaller than the number of test cases in the test suite, in which case
    /// the judgee did not pass the last test case in the returned slice.
    pub fn test_suite(&self) -> &[TestCaseResult] {
        &self.test_suite
    }
}

/// Result of a judge task on a specific test case.
pub struct TestCaseResult {
    /// Verdict of the test case.
    pub verdict: Verdict,

    /// Exit status of the judgee.
    pub judgee_exit_status: ProcessExitStatus,

    /// Exit status of the checker, if any.
    pub checker_exit_status: Option<ProcessExitStatus>,

    /// Exit status of the interactor, if any.
    pub interactor_exit_status: Option<ProcessExitStatus>,

    /// Resource usage statistics of the judgee during its execution.
    pub rusage: ProcessResourceUsage,

    /// Comment made by the answer checker or interactor, if any.
    pub comment: Option<String>,

    /// View into the input file of the test case, if any.
    pub input_view: Option<String>,

    /// View into the answer file of the test case, if any.
    pub answer_view: Option<String>,

    /// View into the output produced by the judgee, if any.
    pub output_view: Option<String>,
}

/// Verdict of the judge.
#[derive(Clone, Copy)]
pub enum Verdict {
    /// The judgee accepted all test cases in the test suite.
    Accepted,

    /// The judgee produced wrong answer on some test case in the test suite.
    WrongAnswer,

    /// The judgee occured a runtime error.
    RuntimeError,

    /// The judgee ran out of CPU time.
    TimeLimitExceeded,

    /// The judgee ran out of memory space.
    MemoryLimitExceeded,

    /// The judgee ran out of real time.
    IdlenessLimitExceeded,

    /// The judgee called an unexpected system call.
    BannedSystemCall,

    /// The checker failed, so judge cannot continue.
    CheckerFailed,

    /// The interactor failed, so judge cannot continue.
    InteractorFailed
}

impl Verdict {
    /// Determine whether this `Verdict` value is `Verdict::Accepted`.
    pub fn is_accepted(&self) -> bool {
        match self {
            Verdict::Accepted => true,
            _ => false
        }
    }

    /// If this `Verdict` is `Verdict::Accepted`, then returns `rhs`; otherwise
    /// returns `self`.
    pub fn and(mut self, rhs: Verdict) -> Verdict {
        self &= rhs;
        self
    }
}

impl BitAnd for Verdict {
    type Output = Self;

    fn bitand(self, rhs: Self) -> <Self as BitAnd>::Output {
        self.and(rhs)
    }
}

impl BitAndAssign for Verdict {
    fn bitand_assign(&mut self, rhs: Verdict) {
        if self.is_accepted() {
            *self = rhs;
        }
    }
}

//! This crate implements the core logic of the judge.
//!

extern crate error_chain;
extern crate log;
extern crate libc;
extern crate nix;
extern crate tempfile;
extern crate sandbox;
extern crate libloading;

#[cfg(feature = "serde")]
extern crate serde;

pub mod engine;
pub mod languages;

use std::ops::{BitAnd, BitAndAssign};
use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use sandbox::{MemorySize, ProcessResourceUsage, ProcessExitStatus};

use languages::LanguageIdentifier;


error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        Sandbox(::sandbox::Error, ::sandbox::ErrorKind);
    }

    foreign_links {
        Io(::std::io::Error);
        Nix(::nix::Error);
    }

    errors {
        LanguageNotFound(lang: LanguageIdentifier) {
            description("language could not be found")
            display("language could not be found: {}", lang)
        }

        LanguageError(message: String) {
            description("language error")
            display("language error: {}", message)
        }
    }
}


/// Describe a compilation task.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CompilationTaskDescriptor {
    /// The program to be compiled.
    pub program: Program,

    /// The kind of the program.
    pub kind: ProgramKind,

    /// The optional output directory.
    pub output_dir: Option<PathBuf>,
}

impl CompilationTaskDescriptor {
    /// Create a new `CompilationTaskDescriptor` instance.
    pub fn new(program: Program) -> Self {
        CompilationTaskDescriptor {
            program,
            kind: ProgramKind::Judgee,
            output_dir: None
        }
    }
}

/// Represent the result of a compilation job.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CompilationResult {
    /// Is the compilation job successful?
    pub succeeded: bool,

    /// The output message generated by the compiler, if any.
    pub compiler_out: Option<String>,

    /// Path to the output file, if any.
    pub output_file: Option<PathBuf>
}

impl CompilationResult {
    /// Create a `CompilationResult` instance representing a successful compilation result.
    pub fn succeed<T>(output_file: T) -> CompilationResult
        where T: Into<PathBuf> {
        CompilationResult {
            succeeded: true,
            compiler_out: None,
            output_file: Some(output_file.into())
        }
    }

    /// Create a `CompilationResult` instance representing an unsuccessful compilation result.
    pub fn fail<T>(compiler_out: T) -> CompilationResult
        where T: Into<String> {
        CompilationResult {
            succeeded: false,
            compiler_out: Some(compiler_out.into()),
            output_file: None
        }
    }
}

/// Describe a judge task.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct JudgeTaskDescriptor {
    /// Program to be judged (called the judgee).
    pub program: Program,

    /// Judge mode.
    pub mode: JudgeMode,

    /// Resource limits.
    pub limits: ResourceLimits,

    /// The test suite, consisting of multiple test cases described by a 2-tuple (input_file,
    /// output_file).
    pub test_suite: Vec<TestCaseDescriptor>,
}

impl JudgeTaskDescriptor {
    /// Create a new `JudgeTaskDescriptor` instance.
    pub fn new(program: Program) -> Self {
        JudgeTaskDescriptor {
            program,
            mode: JudgeMode::default(),
            limits: ResourceLimits::default(),
            test_suite: Vec::new()
        }
    }
}

/// Represent a program stored in local disk file, along with the corresponding language
/// environment. The program file may either be a source file or an executable file.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Program {
    /// Path to the program file.
    pub file: PathBuf,

    /// Language and corresponding branch in which the program is written.
    pub language: LanguageIdentifier,
}

impl Program {
    /// Create a new `Program` value.
    pub fn new<P>(file: P, language: LanguageIdentifier) -> Self
        where P: Into<PathBuf> {
        Program {
            file: file.into(),
            language
        }
    }
}

/// Represent the kind of a program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ProgramKind {
    /// The program is a judgee.
    Judgee,

    /// The program is a checker.
    Checker,

    /// The program is an interactor.
    Interactor
}

impl ProgramKind {
    /// Determine if the execution is an execution of a jury program.
    pub fn is_jury(&self) -> bool {
        use ProgramKind::*;
        match self {
            Checker | Interactor => true,
            _ => false
        }
    }
}

/// Resource limits that should be applied to the judgee when executing judge.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ResourceLimits {
    /// CPU time limit.
    pub cpu_time_limit: Duration,

    /// Real time limit.
    pub real_time_limit: Duration,

    /// Memory limit.
    pub memory_limit: MemorySize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits {
            cpu_time_limit: Duration::from_secs(1),
            real_time_limit: Duration::from_secs(3),
            memory_limit: MemorySize::MegaBytes(256)
        }
    }
}

/// Represent built-in answer checkers used in standard judge mode.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BuiltinCheckers {
    /// The default built-in checker.
    Default,

    /// The floating point aware built-in checker.
    FloatingPointAware,

    /// The case insensitive built-in checker.
    CaseInsensitive
}

impl Default for BuiltinCheckers {
    fn default() -> Self {
        BuiltinCheckers::Default
    }
}

/// The judge mode.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum JudgeMode {
    /// Standard judge mode. The input of the judgee is redirected to the input file of each test
    /// case, and the output of the judgee is compared against the answer file of corresponding test
    /// case by the specified built-in answer checker.
    Standard(BuiltinCheckers),

    /// Special judge mode. The input of the judgee is redirected to the input file of each test
    /// case, and the output of the judgee, together with the input and answer of the test case, are
    /// sent to a user provided program given in the variant field who is responsible for checking
    /// the correctness of the answer.
    SpecialJudge(Program),

    /// Interactive mode. The input and output of the judgee is piped from / to a user provided
    /// program called the interactor. The input and answer of the test case is sent into the
    /// interactor, too. The interator is responsible for checking the correctness of the behavior
    /// of the judgee.
    Interactive(Program)
}

impl Default for JudgeMode {
    fn default() -> Self {
        JudgeMode::Standard(BuiltinCheckers::Default)
    }
}

/// Describe a test case.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TestCaseDescriptor {
    /// Path to the input file.
    pub input_file: PathBuf,

    /// Path to the answer file.
    pub answer_file: PathBuf
}

impl TestCaseDescriptor {
    /// Create a new `TestCaseDescriptor` value.
    pub fn new<P1, P2>(input_file: P1, answer_file: P2) -> Self
        where P1: Into<PathBuf>, P2: Into<PathBuf> {
        TestCaseDescriptor {
            input_file: input_file.into(),
            answer_file: answer_file.into(),
        }
    }
}

/// Result of a judge task.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct JudgeResult {
    /// Overall verdict of the judge task.
    pub verdict: Verdict,

    /// Overall resource usage statistics.
    pub rusage: ProcessResourceUsage,

    /// Judge results of every executed test cases in the test suite. Do not directly modify this
    /// field; use the `add_test_case_result` function instead to maintain `verdict` and `rusage`
    /// accordingly.
    pub test_suite: Vec<TestCaseResult>
}

impl JudgeResult {
    /// Create an empty `JudgeResult` instance.
    pub fn new() -> Self {
        JudgeResult {
            verdict: Verdict::Accepted,
            rusage: ProcessResourceUsage::new(),
            test_suite: Vec::new()
        }
    }

    /// Add the given judge result on some test case to the overall judge result. This function will
    /// maintain the `verdict` and `rusage` field accordingly.
    pub fn add_test_case_result(&mut self, result: TestCaseResult) {
        self.verdict &= result.verdict;
        self.rusage.update(&result.rusage);
        self.test_suite.push(result);
    }
}

impl Default for JudgeResult {
    fn default() -> Self {
        JudgeResult::new()
    }
}

/// Result of a judge task on a specific test case.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

    /// View into the error contents produced by the judgee, if any.
    pub error_view: Option<String>,
}

impl TestCaseResult {
    /// Create a new `TestCaseResult` instance.
    pub fn new() -> Self {
        TestCaseResult {
            verdict: Verdict::Accepted,
            judgee_exit_status: ProcessExitStatus::NotExited,
            checker_exit_status: None,
            interactor_exit_status: None,
            rusage: ProcessResourceUsage::new(),
            comment: None,
            input_view: None,
            answer_view: None,
            output_view: None,
            error_view: None
        }
    }

    /// Set the judgee's exit status. This function also maintains the `verdict` field accordingly.
    ///
    /// This function panics if the given exit status is either `ProcessExitStatus::NotExited`.
    fn set_judgee_exit_status(&mut self, status: ProcessExitStatus) {
        self.judgee_exit_status = status;
        self.verdict = match self.judgee_exit_status {
            ProcessExitStatus::Normal(..) => Verdict::Accepted,
            ProcessExitStatus::KilledBySignal(..) => Verdict::RuntimeError,
            ProcessExitStatus::CPUTimeLimitExceeded => Verdict::TimeLimitExceeded,
            ProcessExitStatus::RealTimeLimitExceeded => Verdict::IdlenessLimitExceeded,
            ProcessExitStatus::MemoryLimitExceeded => Verdict::MemoryLimitExceeded,
            ProcessExitStatus::BannedSyscall => Verdict::BannedSystemCall,
            ProcessExitStatus::NotExited => panic!("unexpected judgee exit status."),
        };
    }
}

/// Verdict of the judge.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

    /// If this `Verdict` is `Verdict::Accepted`, then returns `rhs`; otherwise returns `self`.
    pub fn and(mut self, rhs: Self) -> Self {
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

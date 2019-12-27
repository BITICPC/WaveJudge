//! This module implements the very core logic of the judge, or the engine's
//! logic. The judge engine performs judge task described in
//! `JudgeTaskDescriptor` values and produce judge result in `JudgeResult`
//! values.
//!

mod checkers;
mod io;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use sandbox::{
    MemorySize,
    UserId,
    SystemCall,
    ProcessBuilder,
    ProcessBuilderMemento,
    ProcessExitStatus,
};

use tempfile::{TempDir, NamedTempFile};

use crate::{Error, ErrorKind, Result};
use super::{
    Program,
    ProgramKind,
    CompilationTaskDescriptor,
    CompilationResult,
    JudgeTaskDescriptor,
    JudgeMode,
    BuiltinCheckers,
    TestCaseDescriptor,
    JudgeResult,
    TestCaseResult,
    Verdict
};
use super::languages::{
    LanguageIdentifier,
    LanguageManager,
    LanguageProvider,
    ExecutionInfo,
    CompilationInfo,
};
use checkers::{Checker, CheckerContext};
use io::{
    FileExt,
    TokenizedReader,
};

/// Configuration for a judge engine instance.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct JudgeEngineConfig {
    /// The effective user ID of the judgee, answer checker and interactor.
    pub judge_uid: Option<UserId>,

    /// The directory inside which the judge task will be executed. Every judge task will create a
    /// temporary directory inside this directory and thus every judge task is independent from
    /// each other in the file system's perspective.
    pub judge_dir: Option<PathBuf>,

    /// System call whitelist for the judgee process.
    pub judgee_syscall_whitelist: Vec<SystemCall>,

    /// CPU time limit of answer checkers and interactors.
    pub jury_cpu_time_limit: Option<Duration>,

    /// Real time limit of checkers and interactors.
    pub jury_real_time_limit: Option<Duration>,

    /// Memory limit of answer checkers and interactors.
    pub jury_memory_limit: Option<MemorySize>,

    /// System call whitelist of answer checkers and interactors.
    pub jury_syscall_whitelist: Vec<SystemCall>,
}

impl JudgeEngineConfig {
    /// Create a new `JudgeEngineConfig` instance.
    pub fn new() -> Self {
        JudgeEngineConfig {
            judge_uid: None,
            judge_dir: None,
            judgee_syscall_whitelist: Vec::new(),
            jury_cpu_time_limit: None,
            jury_real_time_limit: None,
            jury_memory_limit: None,
            jury_syscall_whitelist: Vec::new(),
        }
    }
}

/// Provide extension functions for `ExecutionInfo` to convert `ExecutionInfo` values into
/// corresponding `ProcessBuilder` object.
trait ExecutionInfoExt {
    /// Create a `ProcessBuilder` instance from this value.
    fn build(&self) -> Result<ProcessBuilder>;
}

impl ExecutionInfoExt for ExecutionInfo {
    fn build(&self) -> Result<ProcessBuilder> {
        let mut builder = ProcessBuilder::new(self.executable.clone());
        for arg in self.args.iter() {
            builder.add_arg(arg.clone())?;
        }
        for (name, value) in self.envs.iter() {
            builder.add_env(name.clone(), value.clone())?;
        }
        for syscall in self.syscall_whitelist.iter() {
            builder.syscall_whitelist.push(syscall.clone());
        }

        Ok(builder)
    }
}

impl ExecutionInfoExt for CompilationInfo {
    fn build(&self) -> Result<ProcessBuilder> {
        self.compiler.build()
    }
}

/// A judge engine instance.
pub struct JudgeEngine {
    /// Atomic shared reference to the singleton `LanguageManager` instance.
    languages: Arc<LanguageManager>,

    /// Configuration of the judge engine.
    pub config: JudgeEngineConfig,
}

// This implementation block implements creation logic of `JudgeEngine`.
impl JudgeEngine {
    /// Create a new `JudgeEngine` object.
    pub fn new() -> Self {
        JudgeEngine {
            languages: Arc::new(LanguageManager::new()),
            config: JudgeEngineConfig::new(),
        }
    }

    /// Create a new `JudgeEngine` object using the given configuration.
    pub fn with_config(config: JudgeEngineConfig) -> Self {
        JudgeEngine {
            languages: Arc::new(LanguageManager::new()),
            config,
        }
    }

    /// Get the language manager contained in this judge engine.
    pub fn languages<'s>(&'s self) -> &'s LanguageManager {
        &self.languages
    }
}

// This implementation block implements some common facilities used in judge engine.
impl JudgeEngine {
    /// Find a language provider capable of handling the given language environment in current
    /// `JudgeEngine` instance.
    fn find_language_provider(&self, lang: &LanguageIdentifier)
        -> Result<Arc<Box<dyn LanguageProvider>>> {
        self.languages.find(lang)
            .ok_or_else(|| Error::from(ErrorKind::LanguageNotFound(lang.clone())))
    }
}

// This implementation block implements compilation related facilities of `JudgeEngine`.
impl JudgeEngine {
    /// Execute the given compilation task.
    pub fn compile(&self, task: CompilationTaskDescriptor) -> Result<CompilationResult> {
        log::trace!("Compilation task: {:?}", task);

        let compile_info = self.get_compile_info(&task.program, task.kind, task.output_dir)?;
        log::trace!("Compilation info: {:?}", compile_info);

        match compile_info {
            Some(info) => self.execute_compiler(info),
            None => Ok(CompilationResult::succeed(task.program.file))
        }
    }

    /// Get necessary compilation information for compiling the given program of the given kind.
    /// This function can return `Ok(None)` to indicate that the given program need not to be
    /// compiled before execution.
    fn get_compile_info(&self,
        program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> Result<Option<CompilationInfo>> {
        let lang_provider = self.find_language_provider(&program.language)?;
        if lang_provider.metadata().interpreted {
            // This language is an interpreted language and source code do not need to be compiled
            // before execution.
            Ok(None)
        } else {
            lang_provider.compile(program, kind, output_dir)
                .map(|info| Some(info))
                .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
        }
    }

    /// Execute the compiler configuration specified in the given `CompilationInfo` instance.
    fn execute_compiler(&self, compile_info: CompilationInfo) -> Result<CompilationResult> {
        let mut process_builder = compile_info.build()?;
        process_builder.inherit_envs();

        // Redirect `stderr` of the compiler to a pipe.
        let (mut stderr_pipe_read, stderr_pipe_write) = io::pipe()?;
        process_builder.redirections.stderr = Some(stderr_pipe_write);

        // Launch the compiler process.
        let mut process_handle = process_builder.start()?;
        process_handle.wait_for_exit()?;

        let exit_status = process_handle.exit_status();
        log::trace!("Compiler exited with status: {:?}", exit_status);

        match exit_status {
            ProcessExitStatus::Normal(0) =>
                Ok(CompilationResult::succeed(compile_info.output_file.clone())),
            _ => {
                // Read all contents from stderr of the compiler.
                let mut err_msg = String::new();
                stderr_pipe_read.read_to_string(&mut err_msg)?;

                Ok(CompilationResult::fail(err_msg))
            }
        }
    }
}

/// This implementation block implements judge logic of `JudgeEngine`.
impl JudgeEngine {
    /// Execute the given judge task.
    pub fn judge(&self, task: JudgeTaskDescriptor) -> Result<JudgeResult> {
        let judgee_lang_prov = self.find_language_provider(&task.program.language)?;

        // Get execution information of the judgee.
        log::trace!("Judge task: {:?}", task);
        let judgee_exec_info = judgee_lang_prov.execute(&task.program, ProgramKind::Judgee)
            .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
            ?;
        log::trace!("Judgee execution info returned by language provider: {:?}", judgee_exec_info);

        // Apply judge engine configuration to the judgee's builder.
        let mut judgee_bdr = judgee_exec_info.build()?;
        self.apply_judgee_bdr_config(&mut judgee_bdr);

        // Set judgee's resource limits.
        judgee_bdr.limits.cpu_time_limit = Some(task.limits.cpu_time_limit);
        judgee_bdr.limits.real_time_limit = Some(task.limits.real_time_limit);
        judgee_bdr.limits.memory_limit = Some(task.limits.memory_limit);

        // Create a temporary directory for this judge task.
        let judge_dir = match self.config.judge_dir {
            Some(ref parent) => tempfile::tempdir_in(parent)?,
            None => tempfile::tempdir()?
        };
        // And set the judge directory to the judgee's process builder.
        judgee_bdr.dir.root_dir = Some(judge_dir.path().to_owned());
        judgee_bdr.dir.working_dir = Some(judge_dir.path().to_owned());

        // Save the judgee's process builder into a memento.
        let judgee_bdr_mem: ProcessBuilderMemento = judgee_bdr.into();
        log::trace!("Judgee process builder memento built: {:?}", judgee_bdr_mem);

        // Create judge context.
        let context = match task.mode {
            JudgeMode::Standard(checker) => {
                let builtin_checker = self.get_builtin_checker(checker);
                JudgeContext::standard(&task, judge_dir, judgee_bdr_mem, builtin_checker)
            },
            JudgeMode::SpecialJudge(..) | JudgeMode::Interactive(..) => {
                let jury_exec_info = match task.mode {
                    JudgeMode::SpecialJudge(ref checker) =>
                        self.get_execution_info(checker, ProgramKind::Checker)?,
                    JudgeMode::Interactive(ref interactor) =>
                        self.get_execution_info(interactor, ProgramKind::Interactor)?,
                    _ => unreachable!()
                };
                log::trace!("Jury execution info: {:?}", jury_exec_info);

                let mut jury_bdr = jury_exec_info.build()?;
                self.apply_jury_bdr_config(&mut jury_bdr);
                jury_bdr.dir.working_dir = Some(judge_dir.path().to_owned());
                jury_bdr.dir.root_dir = Some(judge_dir.path().to_owned());

                let jury_bdr_mem: ProcessBuilderMemento = jury_bdr.into();
                log::trace!("Jury process builder memento built: {:?}", jury_bdr_mem);

                JudgeContext::with_jury(&task, judge_dir, judgee_bdr_mem, jury_bdr_mem)
            }
        };

        let mut judge_exec = JudgeEngineExecutor::new();
        context.execute(&mut judge_exec)
    }

    /// Apply judgee related configurations to the given `ProcessBuilder` that builds the judgee
    /// process.
    fn apply_judgee_bdr_config(&self, judgee_bdr: &mut ProcessBuilder) {
        judgee_bdr.add_env("ONLINE_JUDGE", "YES")
            .expect("failed to set ONLINE_JUDGE environment variable for judgee.");

        if self.config.judge_uid.is_some() {
            judgee_bdr.uid = Some(self.config.judge_uid.unwrap());
        }

        for syscall in &self.config.judgee_syscall_whitelist {
            judgee_bdr.syscall_whitelist.push(syscall.clone());
        }
    }

    /// Apply jury related configurations to the given `ProcessBuilder` that builds the jury
    /// process.
    fn apply_jury_bdr_config(&self, jury_bdr: &mut ProcessBuilder) {
        jury_bdr.add_env("ONLINE_JUDGE", "YES")
            .expect("failed to set ONLINE_JUDGE environment variable for jury.");

        if self.config.jury_cpu_time_limit.is_none() {
            jury_bdr.limits.cpu_time_limit = self.config.jury_cpu_time_limit;
        }
        if self.config.jury_real_time_limit.is_some() {
            jury_bdr.limits.real_time_limit = self.config.jury_real_time_limit;
        }
        if self.config.jury_memory_limit.is_some() {
            jury_bdr.limits.memory_limit = self.config.jury_memory_limit;
        }

        for syscall in &self.config.jury_syscall_whitelist {
            jury_bdr.syscall_whitelist.push(syscall.clone());
        }
    }

    /// Get a `Checker` trait object corresponding to the given builtin checker indicator.
    fn get_builtin_checker(&self, checker: BuiltinCheckers) -> Checker {
        checkers::get_checker(checker)
    }

    /// Get necessary execution information for executing the given program.
    fn get_execution_info(&self, program: &Program, kind: ProgramKind)
        -> Result<ExecutionInfo> {
        let lang_provider = self.find_language_provider(&program.language)?;
        lang_provider.execute(program, kind)
            .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
    }
}

/// Provide context information about a running judge task.
struct JudgeContext<'a> {
    /// The judge task under execution.
    task: &'a JudgeTaskDescriptor,

    /// Path to the directory inside which the judge task will be executed.
    judge_dir: TempDir,

    /// Process builder memento for the judgee process.
    judgee_bdr: ProcessBuilderMemento,

    /// The built-in checker to be used.
    builtin_checker: Option<Checker>,

    /// Process builder memento for the jury process.
    jury_bdr: Option<ProcessBuilderMemento>,
}

impl<'a> JudgeContext<'a> {
    /// Create a `JudgeContext` instance representing context for a judge task whose judge mode is
    /// `Standard`.
    fn standard(
        task: &'a JudgeTaskDescriptor,
        judge_dir: TempDir,
        judgee_bdr: ProcessBuilderMemento,
        builtin_checker: Checker) -> Self {
        JudgeContext {
            task,
            judge_dir,
            judgee_bdr,
            builtin_checker: Some(builtin_checker),
            jury_bdr: None,
        }
    }

    /// Create a `JudgeContext` instance representing context for a judge task that requires a jury
    /// program.
    fn with_jury(
        task: &'a JudgeTaskDescriptor,
        judge_dir: TempDir,
        judgee_bdr: ProcessBuilderMemento,
        jury_bdr: ProcessBuilderMemento) -> Self {
        JudgeContext {
            task,
            judge_dir,
            judgee_bdr,
            builtin_checker: None,
            jury_bdr: Some(jury_bdr),
        }
    }

    /// Execute the judge task contained in this `JudgeContext` using the given executor.
    fn execute<E>(&self, executor: &mut E) -> Result<JudgeResult>
        where E: ?Sized + TestCaseExecutor {
        let mut res = JudgeResult::new();

        for tc in &self.task.test_suite {
            log::trace!("Judging on test case: (\"{}\", \"{}\")",
                tc.input_file.display(), tc.answer_file.display());
            let mut tc_ctx = TestCaseContext::new(self, tc);

            executor.before(&mut tc_ctx)?;
            match self.task.mode {
                JudgeMode::Standard(..) => {
                    executor.judge_std(&mut tc_ctx)?;
                },
                JudgeMode::SpecialJudge(..) => {
                    executor.judge_spj(&mut tc_ctx)?;
                },
                JudgeMode::Interactive(..) => {
                    executor.judge_interactive(&mut tc_ctx)?;
                }
            };
            executor.after(&mut tc_ctx)?;

            res.add_test_case_result(tc_ctx.result);
        }

        Ok(res)
    }
}

/// Provide judge context on a specific test case.
struct TestCaseContext<'a, 'b> {
    /// The judge context object.
    judge_context: &'a JudgeContext<'b>,

    /// The test case descriptor.
    test_case: &'b TestCaseDescriptor,

    /// The judge result on this test case.
    result: TestCaseResult,
}

impl<'a, 'b> TestCaseContext<'a, 'b> {
    /// Create a new `TestCaseDescriptor` object.
    fn new(judge_context: &'a JudgeContext<'b>, test_case: &'b TestCaseDescriptor) -> Self {
        TestCaseContext {
            judge_context,
            test_case,
            result: TestCaseResult::new(),
        }
    }
}

// Populate data view of input file and answer file into the test case result.
const DATA_VIEW_LEN: usize = 200;

/// Provide a trait that executes judge on a specific test case.
trait TestCaseExecutor {
    /// Called before a test case is executed.
    fn before<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()> {
        let input_view = io::read_file_view(&context.test_case.input_file, DATA_VIEW_LEN)?;
        let answer_view = io::read_file_view(&context.test_case.answer_file, DATA_VIEW_LEN)?;
        context.result.input_view = Some(input_view);
        context.result.answer_view = Some(answer_view);

        Ok(())
    }

    /// Execute standard judge mode on the given judge context.
    fn judge_std<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()>;

    /// Execute special judge mode on the given judge context.
    fn judge_spj<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()>;

    /// Execute interactive judge mode on the given judge context.
    fn judge_interactive<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()>;

    /// Called after a test case is executed.
    fn after<'s, 'a, 'b, 'c>(&'s mut self, _context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()> {
        Ok(())
    }
}

/// Provide an `Executor` for the judge engine.
struct JudgeEngineExecutor;

impl JudgeEngineExecutor {
    /// Create a new `JudgeEngineExecutor` value.
    fn new() -> Self {
        JudgeEngineExecutor { }
    }
}

impl JudgeEngineExecutor {
    /// Execute the judgee program and returns the output file generated by the judgee program.
    /// This function returns `Err` to indicate any errors in the judge, returns `Ok(None)` to
    /// indicate that the judgee program itself failed. The file pointer of the returned
    /// `NamedTempFile` is properly reset to the start of the file.
    fn execute_judgee<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<Option<NamedTempFile>> {
        // Redirect input and answer file.
        let input_file = File::open(&context.test_case.input_file)?;
        let mut output_file = NamedTempFile::new_in(&context.judge_context.judge_dir)?;

        let mut judgee_bdr = context.judge_context.judgee_bdr.restore();
        judgee_bdr.redirections.stdin = Some(input_file);
        judgee_bdr.redirections.stdout = Some(output_file.as_file().duplicate()?);
        judgee_bdr.redirections.ignore_stderr()?;

        // Execute the judgee.
        let mut judgee_handle = judgee_bdr.start()?;
        judgee_handle.wait_for_exit()?;
        log::trace!("Judgee exited with status: {:?}", judgee_handle.exit_status());

        // Read view of output data.
        output_file.as_file_mut().seek(SeekFrom::Start(0))?;
        let output_view = io::read_file_view(output_file.path(), DATA_VIEW_LEN)?;
        context.result.output_view = Some(output_view);

        context.result.set_judgee_exit_status(judgee_handle.exit_status());

        if context.result.verdict.is_accepted() {
            output_file.as_file_mut().seek(SeekFrom::Start(0))?;
            Ok(Some(output_file))
        } else {
            Ok(None)
        }
    }
}

impl TestCaseExecutor for JudgeEngineExecutor {
    fn judge_std<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()> {
        let output_file = match self.execute_judgee(context)? {
            Some(f)=> f,
            None => return Ok(())
        };

        // Open input and answer file of the current test case.
        let input_file = File::open(&context.test_case.input_file)?;
        let answer_file = File::open(&context.test_case.answer_file)?;

        let mut checker_context = CheckerContext::new(
            TokenizedReader::new(input_file),
            TokenizedReader::new(answer_file),
            TokenizedReader::new(output_file.into_file()));
        let checker = context.judge_context.builtin_checker
            .expect("failed to unwrap built-in checker pointer");
        let checker_res = checker(&mut checker_context)?;

        context.result.comment = checker_res.comment;
        context.result.verdict = if checker_res.accepted {
            Verdict::Accepted
        } else {
            Verdict::WrongAnswer
        };

        Ok(())
    }

    fn judge_spj<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()> {
        let output_file = match self.execute_judgee(context)? {
            Some(f) => f,
            None => return Ok(())
        };

        let mut checker_bdr = context.judge_context.jury_bdr.as_ref()
            .expect("failed to unwrap jury process builder as checker process builder")
            .restore();

        // Add answer checker specific command line arguments to the process builder.
        // The 3 command line arguments passed to the answer checker are:
        // 1. fd of the input file of the current test case;
        // 2. fd of the answer file of the current test case;
        // 3. fd of the user's output file on the current test case.
        let input_file = File::open(&context.test_case.input_file)?;
        let answer_file = File::open(&context.test_case.answer_file)?;
        checker_bdr.add_arg(format!("\"{}\"", input_file.as_raw_fd()))?;
        checker_bdr.add_arg(format!("\"{}\"", answer_file.as_raw_fd()))?;
        checker_bdr.add_arg(format!("\"{}\"", output_file.as_raw_fd()))?;

        let (mut comment_read, comment_write) = io::pipe()?;
        checker_bdr.redirections.stdout = Some(comment_write);

        // Start the checker process.
        let mut checker_handle = checker_bdr.start()?;
        checker_handle.wait_for_exit()?;
        log::trace!("Answer checker exited with status: {:?}", checker_handle.exit_status());

        let status = checker_handle.exit_status();
        match status {
            ProcessExitStatus::Normal(..) => {
                // Read the checker's comment.
                let mut comment = String::new();
                comment_read.read_to_string(&mut comment)?;

                match status {
                    ProcessExitStatus::Normal(0) => {
                        // Accepted.
                        context.result.verdict = Verdict::Accepted;
                        context.result.comment = Some(comment);
                    },
                    ProcessExitStatus::Normal(..) => {
                        // Rejected.
                        context.result.verdict = Verdict::WrongAnswer;
                        context.result.comment = Some(comment);
                    },
                    _ => unreachable!(),
                }
            },
            ProcessExitStatus::KilledBySignal(sig) => {
                context.result.verdict = Verdict::CheckerFailed;
                context.result.comment = Some(format!("checker killed by signal: {}", sig))
            },
            ProcessExitStatus::CPUTimeLimitExceeded => {
                context.result.verdict = Verdict::CheckerFailed;
                context.result.comment = Some(String::from("checker CPU time limit exceeded"));
            },
            ProcessExitStatus::MemoryLimitExceeded => {
                context.result.verdict = Verdict::CheckerFailed;
                context.result.comment = Some(String::from("checker memory limit exceeded"));
            },
            ProcessExitStatus::RealTimeLimitExceeded => {
                context.result.verdict = Verdict::CheckerFailed;
                context.result.comment = Some(String::from("checker real time limit exceeded"));
            },
            ProcessExitStatus::BannedSyscall => {
                context.result.verdict = Verdict::CheckerFailed;
                context.result.comment = Some(String::from("checker invokes banned system call"));
            },
            _ => unreachable!()
        };

        Ok(())
    }

    fn judge_interactive<'s, 'a, 'b, 'c>(&'s mut self, context: &'c mut TestCaseContext<'a, 'b>)
        -> Result<()> {
        unimplemented!()
    }
}

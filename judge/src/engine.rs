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
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use sandbox::{
    MemorySize,
    UserId,
    SystemCall,
    ProcessBuilder,
    ProcessExitStatus
};

use crate::{Error, ErrorKind, Result};
use super::{
    Program,
    CompilationTaskDescriptor,
    CompilationScheme,
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
    ExecutionScheme
};
use checkers::{Checker, CheckerContext};
use io::{
    ReadExt,
    FileExt,
    TokenizedReader,
    TempFile
};


/// Configuration for a judge engine instance.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct JudgeEngineConfig {
    /// The effective user ID of the judgee, answer checker and interactor.
    pub judge_uid: Option<UserId>,

    /// The directory inside which the judge task will be executed.
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

/// A judge engine instance.
pub struct JudgeEngine {
    /// Atomic shared reference to the singleton `LanguageManager` instance.
    languages: Arc<LanguageManager>,

    /// Configuration of the judge engine.
    pub config: JudgeEngineConfig,
}

impl JudgeEngine {
    /// Create a new judge engine that performs the given judge task.
    pub fn new() -> Self {
        JudgeEngine {
            languages: super::languages::LanguageManager::singleton(),
            config: JudgeEngineConfig::new()
        }
    }

    /// Create a new judge engine configured using the given judge engine configuration.
    pub fn with_config(config: JudgeEngineConfig) -> Self {
        JudgeEngine {
            languages: super::languages::LanguageManager::singleton(),
            config
        }
    }

    /// Find a language provider capable of handling the given language environment in current
    /// `JudgeEngine` instance.
    fn find_language_provider(&self, lang: &LanguageIdentifier)
        -> Result<Arc<Box<dyn LanguageProvider>>> {
        self.languages.find(lang)
            .ok_or_else(|| Error::from(ErrorKind::LanguageNotFound(lang.clone())))
    }

    /// Get necessary compilation information for compiling the given program under the given
    /// scheme. This function can return `Ok(None)` to indicate that the given program need not to
    /// be compiled before execution.
    fn get_compile_info(&self,
        program: &Program, scheme: CompilationScheme, output_dir: Option<PathBuf>)
        -> Result<Option<CompilationInfo>> {
        let lang_provider = self.find_language_provider(&program.language)?;
        if lang_provider.metadata().interpreted {
            // This language is an interpreted language and source code do not need to be compiled
            // before execution.
            Ok(None)
        } else {
            lang_provider.compile(program, output_dir, scheme)
                .map(|info| Some(info))
                .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
        }
    }

    /// Execute the given compilation task.
    pub fn compile(&self, task: CompilationTaskDescriptor) -> Result<CompilationResult> {
        trace!("Compilation task: {:?}", task);

        let compile_info = self.get_compile_info(&task.program, task.scheme, task.output_dir)?;
        trace!("Compilation info: {:?}", compile_info);

        match compile_info {
            Some(info) => self.execute_compiler(info),
            None => Ok(CompilationResult::succeed(task.program.file))
        }
    }

    /// Execute the compiler configuration specified in the given `CompilationInfo` instance.
    fn execute_compiler(&self, compile_info: CompilationInfo) -> Result<CompilationResult> {
        let mut process_builder = compile_info.create_process_builder()?;

        // Redirect `stderr` of the compiler to a pipe.
        let mut stderr_pipe = io::Pipe::new()?;
        process_builder.redirections.stderr = stderr_pipe.take_write_end();

        // Launch the compiler process.
        let mut process_handle = process_builder.start()?;
        process_handle.wait_for_exit()?;

        let exit_status = process_handle.exit_status();
        trace!("Compiler exited with status: {:?}", exit_status);
        match exit_status {
            ProcessExitStatus::Normal(0) =>
                Ok(CompilationResult::succeed(compile_info.output_file.clone())),
            _ => {
                // Read all contents from `stderr_pipe`.
                let mut err_reader = stderr_pipe.take_read_end().unwrap();
                let mut err_msg = String::new();

                // Ignore the result of `read_to_string` here.
                err_reader.read_to_string(&mut err_msg).ok();

                Ok(CompilationResult::fail(err_msg))
            }
        }
    }

    /// Get a `Checker` trait object corresponding to the given builtin checker indicator.
    fn get_builtin_checker(&self, checker: BuiltinCheckers) -> Checker {
        checkers::get_checker(checker)
    }

    /// Get necessary execution information for executing the given program.
    fn get_execution_info(&self, program: &Program, scheme: ExecutionScheme)
        -> Result<ExecutionInfo> {
        let lang_provider = self.find_language_provider(&program.language)?;
        lang_provider.execute(program, scheme)
            .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
    }

    /// Execute the given judge task.
    pub fn judge(&self, task: JudgeTaskDescriptor) -> Result<JudgeResult> {
        let judgee_lang_prov = self.find_language_provider(&task.program.language)?;

        // Get execution information of the judgee.
        trace!("Judge task: {:?}", task);
        let judgee_exec_info = judgee_lang_prov.execute(&task.program, ExecutionScheme::Judgee)
            .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
            ?;
        trace!("Judgee execution info: {:?}", judgee_exec_info);

        // Create judge context.
        let mut context = match task.mode {
            JudgeMode::Standard(checker) => {
                let builtin_checker = self.get_builtin_checker(checker);
                JudgeContext::standard(&task, judgee_exec_info, builtin_checker)
            },
            JudgeMode::SpecialJudge(ref checker) => {
                let checker_exec_info = self.get_execution_info(checker, ExecutionScheme::Checker)?;
                trace!("Checker execution info: {:?}", checker_exec_info);
                JudgeContext::special_judge(&task, judgee_exec_info, checker_exec_info)
            },
            JudgeMode::Interactive(ref interactor) => {
                let interactor_exec_info = self.get_execution_info(
                    interactor, ExecutionScheme::Interactor)?;
                trace!("Interactor execution info: {:?}", interactor_exec_info);
                JudgeContext::interactive(&task, judgee_exec_info, interactor_exec_info)
            }
        };

        self.judge_on_context(&mut context)?;
        Ok(context.result)
    }

    /// Execute judge on the given judge context.
    fn judge_on_context(&self, context: &mut JudgeContext) -> Result<()> {
        for test_case in context.task.test_suite.iter() {
            trace!("Judging on test case {:?}", test_case);

            context.test_case = Some(TestCaseContext::new(test_case));
            self.judge_on_test_case(context)?;

            context.result.add_test_case_result(context.test_case.take().unwrap().result);
            if !context.result.verdict.is_accepted() {
                trace!("Judgee failed test case. Stop judging.");
                break;
            }
        }

        Ok(())
    }

    /// Execute judge on the current test case.
    fn judge_on_test_case(&self, context: &mut JudgeContext<'_>) -> Result<()> {
        let test_case = context.test_case.as_mut().unwrap();
        self.populate_test_case_data_view(test_case.descriptor, &mut test_case.result)?;

        // Dispatch to different judge engine logic according to different judge modes.
        match context.task.mode {
            JudgeMode::Standard(..) => self.judge_std_on_test_case(context)?,
            JudgeMode::SpecialJudge(..) => self.judge_spj_on_test_case(context)?,
            JudgeMode::Interactive(..) => self.judge_itr_on_test_case(context)?
        };

        Ok(())
    }

    /// Length of views into the input, output and error streams produced by the judgee.
    const DATA_VIEW_LENGTH: usize = 200;

    /// Populate the data view into the input file and answer file of the given test case into the
    /// given `TestCaseResult` instance.
    fn populate_test_case_data_view(&self,
        test_case: &TestCaseDescriptor, result: &mut TestCaseResult) -> std::io::Result<()> {
        let mut input_file = File::open(&test_case.input_file)?;
        let mut output_file = File::open(&test_case.output_file)?;
        result.input_view = input_file.read_to_string_lossy(JudgeEngine::DATA_VIEW_LENGTH)?;
        result.output_view = output_file.read_to_string_lossy(JudgeEngine::DATA_VIEW_LENGTH)?;

        Ok(())
    }

    /// Execute judge on the current test case. The judge mode should be standard mode.
    fn judge_std_on_test_case(&self, context: &mut JudgeContext<'_>) -> Result<()> {
        self.execute_judgee_on_test_case(context)?;

        let test_case = context.test_case.as_mut().unwrap();
        if !test_case.result.verdict.is_accepted() {
            return Ok(());
        }

        // We don't care about what the judgee process writes to the error file. Close the error
        // file explicitly.
        test_case.judgee_error_file.take();

        // Reset file pointers and execute the specified built-in answer checker.
        let mut input_file = test_case.input_file.take().unwrap();
        let answer_file = File::open(&test_case.descriptor.output_file)?;
        let mut judgee_output_file = test_case.judgee_output_file.take().unwrap();
        input_file.seek(SeekFrom::Start(0))?;
        judgee_output_file.file.seek(SeekFrom::Start(0))?;

        let mut checker_context = CheckerContext::new(
            TokenizedReader::new(input_file),
            TokenizedReader::new(answer_file),
            TokenizedReader::new(judgee_output_file.file));
        let checker = context.builtin_checker.unwrap();
        let checker_result = checker(&mut checker_context)?;

        trace!("Result of built-in answer checker: {:?}", checker_result);

        test_case.result.comment = checker_result.comment;
        test_case.result.verdict = if checker_result.accepted {
            Verdict::Accepted
        } else {
            Verdict::WrongAnswer
        };

        Ok(())
    }

    /// Execute judge on the given test case. The judge mode should be special judge mode.
    fn judge_spj_on_test_case(&self, context: &mut JudgeContext<'_>) -> Result<()> {
        self.execute_judgee_on_test_case(context)?;

        let test_case = context.test_case.as_ref().unwrap();
        if !test_case.result.verdict.is_accepted() {
            return Ok(());
        }

        self.execute_checker(context)
    }

    /// Execute the judgee program on the current test case. The judgee program is built using the
    /// given `ProcessBuilder` instance. The current test case's judge result value is maintained
    /// accordingly.
    ///
    /// The input file of the test case, output file of the judgee and error file of the judgee is
    /// opened and stored in the context after calling this function. Views into the output file
    /// and error file of the judgee is extracted as well.
    ///
    /// If the judgee program exit successfully and produces meaningful answers, this function
    /// returns `Ok(Some(..))`; otherwise this function returns `Ok(None)` if no error occur.
    fn execute_judgee_on_test_case(&self, context: &mut JudgeContext<'_>) -> Result<()> {
        // Create process builder for the judgee process.
        let mut judgee_proc_bdr = context.judgee_exec_info.create_process_builder()?;

        // Add the `ONLINE_JUDGE` environment variable.
        judgee_proc_bdr.add_env(String::from("ONLINE_JUDGE"), String::from("TRUE")).unwrap();

        // Set resource limits.
        judgee_proc_bdr.limits.cpu_time_limit = Some(context.task.limits.cpu_time_limit);
        judgee_proc_bdr.limits.real_time_limit = Some(context.task.limits.real_time_limit);
        judgee_proc_bdr.limits.memory_limit = Some(context.task.limits.memory_limit);
        judgee_proc_bdr.use_native_rlimit = false;

        // Set effective user ID and syscall whitelist for the judgee.
        judgee_proc_bdr.uid = self.config.judge_uid;
        for syscall in &self.config.judgee_syscall_whitelist {
            judgee_proc_bdr.syscall_whitelist.push(syscall.clone());
        }

        // Set special directories.
        if self.config.judge_dir.is_some() {
            judgee_proc_bdr.dir.working_dir = self.config.judge_dir.clone();
            judgee_proc_bdr.dir.root_dir = self.config.judge_dir.clone();
        }

        // Prepare file descriptors used for redirections.
        let test_case = context.test_case.as_mut().unwrap();
        let input_file = File::open(&test_case.descriptor.input_file)?;
        let output_file = TempFile::new()?;
        let error_file = TempFile::new()?;

        judgee_proc_bdr.redirections.stdin = Some(input_file.duplicate()?);
        judgee_proc_bdr.redirections.stdout = Some(output_file.file.duplicate()?);
        judgee_proc_bdr.redirections.stderr = Some(error_file.file.duplicate()?);

        // Attach these file descriptors onto the current test case context for further use.
        test_case.input_file = Some(input_file);
        test_case.judgee_output_file = Some(output_file);
        test_case.judgee_error_file = Some(error_file);

        // Execute the judgee alone.
        let mut process = judgee_proc_bdr.start()?;
        process.wait_for_exit()?;

        let exit_status = process.exit_status();
        let rusage = process.rusage();
        trace!("Judgee terminated with status: {:?}", exit_status);
        trace!("Judgee's resource usage: {:?}", rusage);

        test_case.result.set_judgee_exit_status(exit_status);
        test_case.result.rusage = rusage;

        // Extract data view into the judgee's output and error contents.
        let output_file = &mut test_case.judgee_output_file.as_mut().unwrap().file;
        let error_file = &mut test_case.judgee_error_file.as_mut().unwrap().file;
        output_file.seek(SeekFrom::Start(0))?;
        error_file.seek(SeekFrom::Start(0))?;
        test_case.result.output_view = output_file.read_to_string_lossy(
            JudgeEngine::DATA_VIEW_LENGTH)?;
        test_case.result.error_view = error_file.read_to_string_lossy(
            JudgeEngine::DATA_VIEW_LENGTH)?;

        Ok(())
    }

    /// Execute user provided answer checker program.
    fn execute_checker(&self, context: &mut JudgeContext) -> Result<()> {
        let mut checker_bdr = context.checker_exec_info.as_ref().unwrap().create_process_builder()?;

        // Apply `ONLINE_JUDGE` environment variable to the checker process.
        checker_bdr.add_env(String::from("ONLINE_JUDGE"), String::from("TRUE")).unwrap();

        // Apply resource constraits to the checker process.
        checker_bdr.limits.cpu_time_limit = self.config.jury_cpu_time_limit;
        checker_bdr.limits.real_time_limit = self.config.jury_real_time_limit;
        checker_bdr.limits.memory_limit = self.config.jury_memory_limit;
        checker_bdr.use_native_rlimit = false;

        // Apply redirections to the checker.
        let mut checker_output = TempFile::new()?;
        checker_bdr.redirections.stdout = Some(checker_output.file.duplicate()?);

        // Set special directories.
        if self.config.judge_dir.is_some() {
            checker_bdr.dir.working_dir = self.config.judge_dir.clone();
            checker_bdr.dir.root_dir = self.config.judge_dir.clone();
        }

        // Pass input file, answer file and judgee's output file to the custom checker via command
        // line arguments.
        let mut test_case = context.test_case.as_mut().unwrap();
        checker_bdr.add_arg(String::from(test_case.descriptor.input_file.to_str().unwrap()))?;
        checker_bdr.add_arg(String::from(test_case.descriptor.output_file.to_str().unwrap()))?;
        checker_bdr.add_arg(String::from(
            test_case.judgee_output_file.as_ref().unwrap().path.to_str().unwrap()))?;

        checker_bdr.uid = self.config.judge_uid;
        for syscall in &self.config.jury_syscall_whitelist {
            checker_bdr.syscall_whitelist.push(syscall.clone());
        }

        // Execute the checker.
        let mut checker_proc = checker_bdr.start()?;
        checker_proc.wait_for_exit()?;

        let exit_status = checker_proc.exit_status();
        trace!("Checker terminated with status: {:?}", exit_status);

        // Read contents of the output stream of the checker process.
        checker_output.file.seek(SeekFrom::Start(0))?;
        let mut checker_output_msg = String::new();
        checker_output.file.read_to_string(&mut checker_output_msg)?;

        match checker_proc.exit_status() {
            ProcessExitStatus::Normal(0) => {
                // Accepted.
                test_case.result.verdict = Verdict::Accepted;
                test_case.result.comment = Some(checker_output_msg);
            },
            ProcessExitStatus::Normal(..) => {
                // Rejected.
                test_case.result.verdict = Verdict::WrongAnswer;
                test_case.result.comment = Some(checker_output_msg);
            },
            ProcessExitStatus::KilledBySignal(sig) => {
                test_case.result.verdict = Verdict::CheckerFailed;
                test_case.result.comment = Some(format!("checker killed by signal: {}", sig))
            },
            ProcessExitStatus::CPUTimeLimitExceeded => {
                test_case.result.verdict = Verdict::CheckerFailed;
                test_case.result.comment = Some(String::from("checker CPU time limit exceeded"));
            },
            ProcessExitStatus::MemoryLimitExceeded => {
                test_case.result.verdict = Verdict::CheckerFailed;
                test_case.result.comment = Some(String::from("checker memory limit exceeded"));
            },
            ProcessExitStatus::RealTimeLimitExceeded => {
                test_case.result.verdict = Verdict::CheckerFailed;
                test_case.result.comment = Some(String::from("checker real time limit exceeded"));
            },
            ProcessExitStatus::BannedSyscall => {
                test_case.result.verdict = Verdict::CheckerFailed;
                test_case.result.comment = Some(String::from("checker invokes banned system call"));
            },
            _ => unreachable!()
        }

        Ok(())
    }

    /// Execute judge on the current test case. The judge mode should be interactive mode.
    fn judge_itr_on_test_case(&self, context: &mut JudgeContext) -> Result<()> {
        // TODO: Implement judge_itr_on_test_case.
        unimplemented!()
    }
}

struct TestCaseContext<'a> {
    /// Currently executing test case.
    descriptor: &'a TestCaseDescriptor,

    /// Input file of currently executing test case.
    input_file: Option<File>,

    /// Answer file of currently executing test case.
    answer_file: Option<File>,

    /// Judgee's output file of currently executing test case.
    judgee_output_file: Option<TempFile>,

    /// Judgee's error file of currently executing test case.
    judgee_error_file: Option<TempFile>,

    /// Result of currently executing test case.
    result: TestCaseResult
}

impl<'a> TestCaseContext<'a> {
    /// Create a new `TestCaseDescriptor` instance.
    fn new(test_case: &'a TestCaseDescriptor) -> Self {
        TestCaseContext {
            descriptor: test_case,
            input_file: None,
            answer_file: None,
            judgee_output_file: None,
            judgee_error_file: None,
            result: TestCaseResult::new()
        }
    }
}

/// Provide context information about a running judge task.
struct JudgeContext<'a> {
    /// The judge task under execution.
    task: &'a JudgeTaskDescriptor,

    /// The execution information about the judgee.
    judgee_exec_info: ExecutionInfo,

    /// The built-in checker to be used.
    builtin_checker: Option<Checker>,

    /// The execution information about the checker.
    checker_exec_info: Option<ExecutionInfo>,

    /// The execution information about the interactor.
    interactor_exec_info: Option<ExecutionInfo>,

    /// Context of currently executing test case.
    test_case: Option<TestCaseContext<'a>>,

    /// Result of the judge task.
    result: JudgeResult,
}

impl<'a> JudgeContext<'a> {
    /// Create a `JudgeContext` instance representing context for a judge task whose judge mode is
    /// `Standard`.
    fn standard(task: &'a JudgeTaskDescriptor, judgee_exec_info: ExecutionInfo,
        builtin_checker: Checker) -> Self {
        JudgeContext {
            task,
            judgee_exec_info,
            builtin_checker: Some(builtin_checker),
            checker_exec_info: None,
            interactor_exec_info: None,
            test_case: None,
            result: JudgeResult::new()
        }
    }

    /// Create a `JudgeContext` instance representing context for a judge task whose judge mode is
    /// `SpecialJudge`.
    fn special_judge(task: &'a JudgeTaskDescriptor, judgee_exec_info: ExecutionInfo,
        checker_exec_info: ExecutionInfo) -> Self {
        JudgeContext {
            task,
            judgee_exec_info,
            builtin_checker: None,
            checker_exec_info: Some(checker_exec_info),
            interactor_exec_info: None,
            test_case: None,
            result: JudgeResult::new()
        }
    }

    /// Create a `JudgeContext` instance representing context for a judge task whose judge mode is
    /// `Interactive`.
    fn interactive(task: &'a JudgeTaskDescriptor, judgee_exec_info: ExecutionInfo,
        interactor_exec_info: ExecutionInfo) -> Self {
        JudgeContext {
            task,
            judgee_exec_info,
            builtin_checker: None,
            checker_exec_info: None,
            interactor_exec_info: Some(interactor_exec_info),
            test_case: None,
            result: JudgeResult::new()
        }
    }
}

/// Provide a trait for structs that can be sent to the execution engine to execute a program in a
/// sandbox.
trait Executable {
    /// Create a `ProcessBuilder` instance from this executable. The create `ProcessBuilder`
    /// instance is used for creating a sandboxed process to execute some program.
    fn create_process_builder(&self) -> Result<ProcessBuilder>;
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
}

impl ExecutionInfo {
    /// Create a new `ExecutionInfo` instance.
    pub fn new<T>(executable: T) -> ExecutionInfo
        where T: Into<PathBuf> {
        ExecutionInfo {
            executable: executable.into(),
            args: Vec::new(),
            envs: Vec::new()
        }
    }
}

impl Executable for ExecutionInfo {
    fn create_process_builder(&self) -> Result<ProcessBuilder> {
        let mut builder = ProcessBuilder::new(self.executable.clone());
        for arg in self.args.iter() {
            builder.add_arg(arg.clone())?;
        }
        for (name, value) in self.envs.iter() {
            builder.add_env(name.clone(), value.clone())?;
        }

        Ok(builder)
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

impl Executable for CompilationInfo {
    fn create_process_builder(&self) -> Result<ProcessBuilder> {
        self.compiler.create_process_builder()
    }
}

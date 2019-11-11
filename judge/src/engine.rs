//! This module implements the very core logic of the judge, or the engine's
//! logic. The judge engine performs judge task described in
//! `JudgeTaskDescriptor` values and produce judge result in `JudgeResult`
//! values.
//!

mod checkers;
mod io;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sandbox::{ProcessBuilder, ProcessExitStatus};

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
};
use super::languages::{
    LanguageIdentifier,
    LanguageManager,
    LanguageProvider
};
use checkers::{
    Checker,
    CheckerContext
};


/// A judge engine instance.
pub struct JudgeEngine {
    /// Atomic shared reference to the singleton `LanguageManager` instance.
    languages: Arc<LanguageManager>,
}

impl JudgeEngine {
    /// Create a new judge engine that performs the given judge task.
    pub fn new() -> JudgeEngine {
        JudgeEngine {
            languages: super::languages::LanguageManager::singleton()
        }
    }

    /// Find a language provider capable of handling the given language
    /// environment in current `JudgeEngine` instance.
    fn find_language_provider(&self, lang: &LanguageIdentifier)
        -> Result<Arc<Box<dyn LanguageProvider>>> {
        self.languages.find(lang).ok_or_else(
            || Error::from(ErrorKind::LanguageNotFound(lang.clone())))
    }

    /// Get necessary compilation information for compiling the given program
    /// under the given scheme. This function can return `Ok(None)` to indicate
    /// that the given program need not to be compiled before execution.
    fn get_compile_info(&self,
        program: &Program, scheme: CompilationScheme, output_dir: Option<&Path>)
        -> Result<Option<CompilationInfo>> {
        let lang_provider = self.find_language_provider(&program.language)?;
        if lang_provider.metadata().interpreted {
            // This language is an interpreted language and source code do not
            // need to be compiled before execution.
            Ok(None)
        } else {
            lang_provider.compile(program, output_dir, scheme)
                .map(|info| Some(info))
                .map_err(
                    |e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
        }
    }

    /// Execute the given compilation task.
    pub fn compile(&self, task: &CompilationTaskDescriptor)
        -> Result<CompilationResult> {
        let output_dir = task.output_dir.as_ref().map(|p| p.as_path());
        let compile_info = self.get_compile_info(
            &task.program, task.scheme, output_dir)?;

        match compile_info {
            Some(info) => self.execute_compiler(&info),
            None => Ok(CompilationResult::succeed(&task.program.file))
        }
    }

    /// Execute the compiler configuration specified in the given
    /// `CompilationInfo` instance.
    fn execute_compiler(&self, compile_info: &CompilationInfo)
        -> Result<CompilationResult> {
        let mut process_builder = compile_info.create_process_builder()?;

        // Redirect `stderr` of the compiler to a pipe.
        let mut stderr_pipe = io::Pipe::new()?;
        process_builder.redirections.stderr = stderr_pipe.take_write_end();

        // Launch the compiler process.
        let mut process_handle = process_builder.start()?;
        process_handle.wait_for_exit()?;

        match process_handle.exit_status() {
            ProcessExitStatus::Normal(0) =>
                Ok(CompilationResult::succeed(&compile_info.output_file)),
            _ => {
                // Read all contents from `stderr_pipe`.
                let mut err_reader = stderr_pipe.take_read_end().unwrap();
                let mut err_msg = String::new();

                // Ignore the result of `read_to_string` here.
                err_reader.read_to_string(&mut err_msg).ok();

                Ok(CompilationResult::fail(&err_msg))
            }
        }
    }

    /// Get necessary execution information for executing the given program.
    fn get_execution_info(&self, program: &Program) -> Result<ExecutionInfo> {
        let lang_provider = self.find_language_provider(&program.language)?;
        lang_provider.execute(program)
            .map_err(|e|
                Error::from(ErrorKind::LanguageError(format!("{}", e))))
    }

    /// Execute the given judge task.
    pub fn judge(&self, task: &JudgeTaskDescriptor) -> Result<JudgeResult> {
        let judgee_lang_prov =
            self.find_language_provider(&task.program.language)?;

        // Get execution information of the judgee.
        let judgee_exec_info = judgee_lang_prov.execute(&task.program)
            .map_err(
                |e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
            ?;
        let mut context = JudgeContext::new(&task, judgee_exec_info);

        // Get execution information of the checker or interactor, if any.
        match task.mode {
            JudgeMode::SpecialJudge(ref checker) =>
                context.checker_exec_info =
                    Some(self.get_execution_info(checker)?),
            JudgeMode::Interactive(ref interactor) =>
                context.interactor_exec_info =
                    Some(self.get_execution_info(interactor)?),
            _ => ()
        };

        self.judge_on_context(&mut context)?;
        Ok(context.result)
    }

    /// Execute judge on the given judge context.
    fn judge_on_context(&self, context: &mut JudgeContext) -> Result<()> {
        for test_case in context.task.test_suite.iter() {
            self.judge_on_test_case(context, test_case)?;
            if !context.result.verdict.is_accepted() {
                break;
            }
        }

        Ok(())
    }

    /// Execute judge on the given test case. The judge result of the test case
    /// will be added to the given judge context.
    fn judge_on_test_case(&self,
        context: &mut JudgeContext, test_case: &TestCaseDescriptor)
        -> Result<()> {
        let mut judgee_proc_bdr =
            context.judgee_exec_info.create_process_builder()?;

        // Apply resource limits to the judgee.
        judgee_proc_bdr.limits.cpu_time_limit =
            Some(context.task.limits.cpu_time_limit);
        judgee_proc_bdr.limits.real_time_limit =
            Some(context.task.limits.real_time_limit);
        judgee_proc_bdr.limits.memory_limit =
            Some(context.task.limits.memory_limit);
        judgee_proc_bdr.use_native_rlimit = false;

        match context.task.mode {
            JudgeMode::Standard(builtin_checker) =>
                self.judge_std_on_test_case(context, builtin_checker, test_case),
            JudgeMode::SpecialJudge(..) =>
                self.judge_spj_on_test_case(context, test_case),
            JudgeMode::Interactive(..) =>
                self.judge_itr_on_test_case(context, test_case)
        }
    }

    /// Execute judge on the given test case. The judge mode should be standard
    /// mode.
    fn judge_std_on_test_case(&self,
        context: &mut JudgeContext, checker: BuiltinCheckers, test_case: &TestCaseDescriptor)
        -> Result<()> {
        // TODO: Implement judge_std_on_test_case.
        unimplemented!()
    }

    /// Execute judge on the given test case. The judge mode should be special
    /// judge mode.
    fn judge_spj_on_test_case(&self,
        context: &mut JudgeContext, test_case: &TestCaseDescriptor)
        -> Result<()> {
        // TODO: Implement judge_std_on_test_case.
        unimplemented!()
    }

    /// Execute judge on the given test case. The judge mode should be
    /// interactive mode.
    fn judge_itr_on_test_case(&self,
        context: &mut JudgeContext, test_case: &TestCaseDescriptor)
        -> Result<()> {
        // TODO: Implement judge_std_on_test_case.
        unimplemented!()
    }
}

/// Provide context information about a running judge task.
struct JudgeContext<'a> {
    /// The judge task under execution.
    task: &'a JudgeTaskDescriptor,

    /// The execution information about the judgee.
    judgee_exec_info: ExecutionInfo,

    /// The execution information about the checker.
    checker_exec_info: Option<ExecutionInfo>,

    /// The execution information about the interactor.
    interactor_exec_info: Option<ExecutionInfo>,

    /// Result of the judge task.
    result: JudgeResult,
}

impl<'a> JudgeContext<'a> {
    /// Create a new `JudgeContext` instance.
    fn new(task: &'a JudgeTaskDescriptor, judgee_exec_info: ExecutionInfo)
        -> JudgeContext<'a> {
        JudgeContext {
            task,
            judgee_exec_info,
            checker_exec_info: None,
            interactor_exec_info: None,
            result: JudgeResult::empty()
        }
    }
}

/// Provide a trait for structs that can be sent to the execution engine to
/// execute a program in a sandbox.
trait Executable {
    /// Create a `ProcessBuilder` instance from this executable. The create
    /// `ProcessBuilder` instance is used for creating a sandboxed process
    /// to execute some program.
    fn create_process_builder(&self) -> Result<ProcessBuilder>;
}

/// Provide necessary information to execute a program.
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
    pub fn new(executable: &Path) -> ExecutionInfo {
        ExecutionInfo {
            executable: executable.to_owned(),
            args: Vec::new(),
            envs: Vec::new()
        }
    }
}

impl Executable for ExecutionInfo {
    fn create_process_builder(&self) -> Result<ProcessBuilder> {
        let mut builder = ProcessBuilder::new(&self.executable);
        for arg in self.args.iter() {
            builder.add_arg(arg)?;
        }
        for (name, value) in self.envs.iter() {
            builder.add_env(name, value)?;
        }

        Ok(builder)
    }
}

/// Provide necessary information to compile a source program.
pub struct CompilationInfo {
    /// Information necessary to execute the compiler instance.
    pub compiler: ExecutionInfo,

    /// Path to the output file generated by the compiler. These files will be
    /// sent to the language provider creating this `CompilerInfo` instance to
    /// execute the program.
    pub output_file: PathBuf
}

impl CompilationInfo {
    /// Create a new `CompilationInfo` instance.
    pub fn new(compiler: &Path, output_file: &Path) -> CompilationInfo {
        CompilationInfo {
            compiler: ExecutionInfo::new(compiler),
            output_file: output_file.to_owned()
        }
    }
}

impl Executable for CompilationInfo {
    fn create_process_builder(&self) -> Result<ProcessBuilder> {
        self.compiler.create_process_builder()
    }
}

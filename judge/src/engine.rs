//! This module implements the very core logic of the judge, or the engine's
//! logic. The judge engine performs judge task described in
//! `JudgeTaskDescriptor` values and produce judge result in `JudgeResult`
//! values.
//!

mod io;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sandbox::{ProcessBuilder, ProcessExitStatus};

use crate::{Error, ErrorKind, Result};
use super::{
    CompilationTaskDescriptor,
    CompilationResult,
    JudgeTaskDescriptor,
    JudgeResult,
};
use super::languages::LanguageManager;


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

    /// Execute the given compilation task.
    pub fn compile(&self, task: &CompilationTaskDescriptor)
        -> Result<CompilationResult> {
        // Find the corresponding language provider to handle this compilation
        // job.
        let lang = &task.program.language;
        let lang_provider = self.languages.find(lang)
            .ok_or_else(|| Error::from(
                ErrorKind::LanguageNotFound(lang.clone())))
            ?;

        let metadata = lang_provider.metadata();
        if metadata.interpreted {
            // This language is an interpreted language and source code do not
            // need to be compiled before execution.
            return Ok(CompilationResult::succeed(&task.program.file));
        }

        // Request the language provider to create compiler related information.
        let output_dir = task.output_dir.as_ref().map(|p| p.as_path());
        let compile_info = lang_provider.compile(
                &task.program, output_dir, task.scheme)
            .map_err(|e| Error::from(ErrorKind::LanguageError(format!("{}", e))))
            ?;

        // Execute the compiler.
        self.execute_compiler(&compile_info)
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

    /// Execute the given judge task.
    pub fn judge(&self, task: JudgeTaskDescriptor)
        -> Result<JudgeResult> {
        unimplemented!()
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

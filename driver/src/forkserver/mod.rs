//! This module implements the forkserver used in WaveJudge.
//!

mod core;
mod io;

use std::fs::File;
use std::sync::Mutex;

use nix::unistd::{Pid, ForkResult};
use nix::sys::signal::Signal;

use serde::{Serialize, Deserialize};

use judge::{
    ProgramKind,
    CompilationTaskDescriptor,
    CompilationResult,
    JudgeTaskDescriptor,
    JudgeResult,
};
use judge::languages::LanguageIdentifier;

use crate::config::JudgeEngineConfig;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
        NixError(::nix::Error);
        SerdeMessagePackSerializationError(::rmp_serde::encode::Error);
        SerdeMessagePackDeserializationError(::rmp_serde::decode::Error);
    }

    links {
        JudgeError(::judge::Error, ::judge::ErrorKind);
    }
}

/// Represent a command to be sent to the fork server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    /// The compile command. The fork server will tries to execute the specified compilation task.
    Compile(CompilationTaskDescriptor),

    /// The judge command. The fork server will tries to execute the specified judge task.
    Judge(JudgeTaskDescriptor),
}

impl From<CompilationTaskDescriptor> for Command {
    fn from(d: CompilationTaskDescriptor) -> Self {
        Command::Compile(d)
    }
}

impl From<JudgeTaskDescriptor> for Command {
    fn from(d: JudgeTaskDescriptor) -> Self {
        Command::Judge(d)
    }
}

impl Into<CompilationTaskDescriptor> for Command {
    fn into(self) -> CompilationTaskDescriptor {
        use Command::*;
        match self {
            Compile(d) => d,
            _ => panic!("current Command is not Compile.")
        }
    }
}

impl Into<JudgeTaskDescriptor> for Command {
    fn into(self) -> JudgeTaskDescriptor {
        use Command::*;
        match self {
            Judge(d) => d,
            _ => panic!("current Command is not Judge.")
        }
    }
}

/// Represent the result of an execution of a command.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommandResult {
    /// The result of a compilation task.
    Compile(CompilationResult),

    /// The result of a judge task.
    Judge(JudgeResult)
}

impl CommandResult {
    pub fn unwrap_as_compilation_result(self) -> CompilationResult {
        use CommandResult::*;
        match self {
            Compile(r) => r,
            _ => panic!("current CommandResult is not Compile.")
        }
    }

    pub fn unwrap_as_judge_result(self) -> JudgeResult {
        use CommandResult::*;
        match self {
            Judge(r) => r,
            _ => panic!("current CommandResult is not Judge.")
        }
    }
}

impl From<CompilationResult> for CommandResult {
    fn from(r: CompilationResult) -> Self {
        CommandResult::Compile(r)
    }
}

impl From<JudgeResult> for CommandResult {
    fn from(r: JudgeResult) -> Self {
        CommandResult::Judge(r)
    }
}

impl Into<CompilationResult> for CommandResult {
    fn into(self) -> CompilationResult {
        self.unwrap_as_compilation_result()
    }
}

impl Into<JudgeResult> for CommandResult {
    fn into(self) -> JudgeResult {
        self.unwrap_as_judge_result()
    }
}

/// Provide fully duplex communication primitives to the fork server.
struct ForkServerSocket {
    /// The read end of the pipe to the fork server.
    reader: File,

    /// The write end of the pipe to the fork server.
    writer: File,
}

impl ForkServerSocket {
    /// Create a new `ForkServerSocket` value from the given pipe.
    fn from_pipes(reader: File, writer: File) -> Self {
        ForkServerSocket { reader, writer }
    }

    /// Send the specified value through the socket.
    fn send<T>(&mut self, cmd: &T) -> Result<()>
        where T: ?Sized + Serialize {
        rmp_serde::encode::write(&mut self.writer, cmd)?;
        Ok(())
    }

    /// Receive a value of the specified type from the socket.
    fn receive<T>(&mut self) -> Result<T>
        where T: for<'de> Deserialize<'de> {
        let value: T = rmp_serde::decode::from_read(&mut self.reader)?;
        Ok(value)
    }
}

/// Represent a fork server socket pair. The socke pair contains two sockets that are internally
/// connected by two anonymous pipes.
struct ForkServerSocketPair(ForkServerSocket, ForkServerSocket);

impl ForkServerSocketPair {
    /// Create a new fork server socket pair.
    fn new() -> Result<Self> {
        let pipe_1 = io::create_pipe()?;
        let pipe_2 = io::create_pipe()?;
        Ok(ForkServerSocketPair(
            ForkServerSocket::from_pipes(pipe_1.reader, pipe_2.writer),
            ForkServerSocket::from_pipes(pipe_2.reader, pipe_1.writer)
        ))
    }
}

/// Provide a client through which one can communicate with the fork server.
pub struct ForkServerClient {
    /// The socket to the fork server.
    socket: Mutex<ForkServerSocket>,

    /// Pid of the fork server.
    pub fork_server_id: Pid,
}

impl ForkServerClient {
    /// Create a new `ForkServerClient` value.
    fn new(socket: ForkServerSocket, fork_server_id: Pid) -> Self {
        ForkServerClient {
            socket: Mutex::new(socket),
            fork_server_id
        }
    }

    /// Execute the given command on the fork server.
    pub fn execute_cmd(&self, cmd: &Command) -> Result<CommandResult> {
        let mut lock = self.socket.lock().expect("failed to lock mutex: poisoned");
        lock.send(cmd)?;
        Ok(lock.receive()?)
    }
}

impl Drop for ForkServerClient {
    fn drop(&mut self) {
        // Kill the fork server process.
        nix::sys::signal::kill(self.fork_server_id, Signal::SIGKILL).ok();
    }
}

/// Provide extension functions for `ForkServerClient`.
pub trait ForkServerClientExt {
    /// Compile the literal source code into executable file.
    fn compile_source<T>(&self, source: &T, lang: LanguageIdentifier, kind: ProgramKind)
        -> Result<CompilationResult>
        where T: ?Sized + AsRef<str>;
}

impl ForkServerClientExt for ForkServerClient {
    fn compile_source<T>(&self, source: &T, lang: LanguageIdentifier, kind: ProgramKind)
        -> Result<CompilationResult>
        where T: ?Sized + AsRef<str> {
        // Create a temp file to store the source code of jury.
        let src_file = tempfile::NamedTempFile::new()?;
        std::fs::write(src_file.path(), source.as_ref())?;

        let program = judge::Program::new(src_file.path(), lang);
        let mut task = judge::CompilationTaskDescriptor::new(program);

        // Create a temp directory for storing the output files of the compilation.
        let output_dir = tempfile::tempdir()?;
        task.output_dir = Some(output_dir.path().to_owned());
        task.kind = kind;

        // Execute the compilation job.
        let cmd = Command::Compile(task);
        let result = self.execute_cmd(&cmd)?.unwrap_as_compilation_result();

        Ok(result)
    }
}

/// Start the fork server.
pub fn start_fork_server(judge_engine_config: &JudgeEngineConfig) -> Result<ForkServerClient> {
    let sock_pair = ForkServerSocketPair::new()?;

    // The first component of sock_pair (`sock_pair.0`) will be passed to the client and the second
    // component of sock_pair (`sock_pair.1`) will be passed to the fork server.
    match nix::unistd::fork()? {
        ForkResult::Parent { child: fork_server_pid } => {
            // Close the second component of sock_pair
            drop(sock_pair.1);
            Ok(ForkServerClient::new(sock_pair.0, fork_server_pid))
        },
        ForkResult::Child => {
            // Close the first component of sock_pair and enter the fork server main.
            drop(sock_pair.0);
            core::fork_server_main(judge_engine_config, sock_pair.1)?;
            unreachable!()
        }
    }
}

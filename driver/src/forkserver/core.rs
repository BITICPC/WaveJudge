//! This module implements the core logic of the fork server.
//!

use std::time::Duration;

use sandbox::{MemorySize, SystemCall};

use judge::{
    CompilationTaskDescriptor,
    CompilationResult,
    JudgeTaskDescriptor,
    JudgeResult,
};
use judge::engine::{
    JudgeEngine,
    JudgeEngineConfig,
};

use super::{Error, Result};

use super::{Command, CommandResult};
use super::ForkServerSocket;

use crate::config::JudgeEngineConfig as AppJudgeEngineConfig;

/// The entry point of the fork server. This function should never returns on normal execution.
pub(super) fn fork_server_main(config: &AppJudgeEngineConfig, mut socket: ForkServerSocket)
    -> Result<()> {
    // TODO: Change the return type of this function from `Result<()>` to `Result<!>` after the
    // TODO: never type `!` stablize.

    log::info!("Starting fork server");
    let handler = CommandHandler::new(config);
    log::info!("Fork server started");

    loop {
        let cmd: Command = socket.receive()?;
        log::debug!("Fork server receives command: {:?}", cmd);
        let res = handler.handle_cmd(cmd)?;
        socket.send(&res)?;
    }
}

/// Get the judge engine configuration from the given application wide judge engine configuration.
fn get_judge_engine_config(app_config: &AppJudgeEngineConfig) -> JudgeEngineConfig {
    let mut engine_config = JudgeEngineConfig::new();

    engine_config.judge_uid = match super::io::lookup_uid(&app_config.judge_username) {
        Ok(Some(uid)) => Some(uid),
        Ok(None) => {
            log::warn!("Cannot lookup user: {}", app_config.judge_username);
            None
        },
        Err(e) => {
            log::error!("Failed to lookup user: {}: {}", app_config.judge_username, e);
            None
        }
    };

    engine_config.judge_dir = Some(app_config.judge_dir.clone());

    fn syscall_convert_and_push<T>(name: T, output: &mut Vec<SystemCall>)
        where T: AsRef<str> {
        let syscall = match SystemCall::from_name(name.as_ref()) {
            Ok(sc) => sc,
            Err(e) => {
                log::error!("Cannot identify system call: {}: {}", name.as_ref(), e);
                return;
            }
        };

        output.push(syscall);
    }

    for syscall_name in &app_config.judgee_syscall_whitelist {
        syscall_convert_and_push(syscall_name, &mut engine_config.judgee_syscall_whitelist);
    }

    engine_config.jury_cpu_time_limit = Some(
        Duration::from_millis(app_config.jury_cpu_time_limit));
    engine_config.jury_real_time_limit = Some(
        Duration::from_millis(app_config.jury_real_time_limit));
    engine_config.jury_memory_limit = Some(
        MemorySize::MegaBytes(app_config.jury_memory_limit));

    for syscall_name in &app_config.jury_syscall_whitelist {
        syscall_convert_and_push(syscall_name, &mut engine_config.jury_syscall_whitelist);
    }

    engine_config
}

/// Implement the command handler used in the fork server. The command handler is just a thin
/// wrapper around `JudgeEngine` that forwards fork server commands to corresponding judge engine
/// invokes.
struct CommandHandler {
    /// The judge engine.
    judge_engine: JudgeEngine,
}

impl CommandHandler {
    /// Create and initializes a new `CommandHandler`.
    fn new(app_config: &AppJudgeEngineConfig) -> Self {
        let engine_config = get_judge_engine_config(app_config);
        let engine = JudgeEngine::with_config(engine_config);

        log::info!("Loading language provider dynamic libraries");
        for lang_so in &app_config.language_dylibs {
            match engine.languages().load_dylib(lang_so) {
                Ok(..) => (),
                Err(e) => {
                    log::error!("Failed to load langauge dylib: \"{}\": {}", lang_so.display(), e);
                }
            };
        }

        CommandHandler {
            judge_engine: engine
        }
    }

    /// Execute the given command.
    fn handle_cmd(&self, cmd: Command) -> Result<CommandResult> {
        match cmd {
            Command::Compile(task) => {
                let task_result = self.handle_compile_task(task)?;
                Ok(CommandResult::from(task_result))
            },
            Command::Judge(task) => {
                let task_result = self.handle_judge_task(task)?;
                Ok(CommandResult::from(task_result))
            },
        }
    }

    /// Execute the given compilation command, using the judge engine contained in this handler.
    fn handle_compile_task(&self, task: CompilationTaskDescriptor) -> Result<CompilationResult> {
        self.judge_engine.compile(task).map_err(Error::from)
    }

    /// Execute the given judge command, using the judge engine contained in this handler.
    fn handle_judge_task(&self, task: JudgeTaskDescriptor) -> Result<JudgeResult> {
        self.judge_engine.judge(task).map_err(Error::from)
    }
}

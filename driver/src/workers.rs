//! This module implements the worker threads.
//!

use std::any::Any;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rand::Rng;

use crate::AppContext;

use crate::forkserver::{ForkServerClientExt, Command as ForkServerCommand};
use crate::restful::entities::{SubmissionInfo, JudgeMode, SubmissionJudgeResult, Verdict};

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        ArchivesError(crate::storage::archives::Error, crate::storage::archives::ErrorKind);
        ProblemsError(crate::storage::problems::Error, crate::storage::problems::ErrorKind);
        ForkServerError(crate::forkserver::Error, crate::forkserver::ErrorKind);
    }

    errors {
        InvalidNumberOfWorkers {
            description("invalid number of workers.")
        }

        WorkerFailed { worker_id: u32, e: Box<dyn Any + Send> } {
            description("Worker thread failed.")
            display("Worker thread #{} failed.", worker_id)
        }
    }
}

/// Provide extension functions for `SubmissionJudgeResult`.
trait SubmissionJudgeResultExt {
    /// Create a `SubmissionJudgeResult` value representing a failed judge result.
    fn failure<T>(message: T) -> Self
        where T: Into<String>;

    /// Create a `SubmissionJudgeResult` value representing a compilation failed judge result.
    fn compilation_failed<T>(message: T) -> Self
        where T: Into<String>;

    /// Create a `SubmissionJudgeResult` value representing a failed judge attempt because the
    /// checker cannot be compiled successfully.
    fn checker_compilation_failed() -> Self;

    /// Create a `SubmissionJudgeResult` value representing a failed judge attempt because the
    /// interactor cannot be compiled successfully.
    fn interactor_compilation_failed() -> Self;
}

impl SubmissionJudgeResultExt for SubmissionJudgeResult {
    fn failure<T>(message: T) -> Self
        where T: Into<String> {
        SubmissionJudgeResult {
            verdict: Verdict::JudgeFailed,
            compiler_message: message.into(),
            time: 0,
            memory: 0,
            test_cases: Vec::new(),
        }
    }

    fn compilation_failed<T>(message: T) -> Self
        where T: Into<String> {
        SubmissionJudgeResult {
            verdict: Verdict::CompilationFailed,
            ..Self::failure(message)
        }
    }

    fn checker_compilation_failed() -> Self {
        SubmissionJudgeResult {
            verdict: Verdict::CheckerCompilationFailed,
            ..Self::failure("")
        }
    }

    fn interactor_compilation_failed() -> Self {
        SubmissionJudgeResult {
            verdict: Verdict::InteractorCompilationFailed,
            ..Self::failure("")
        }
    }
}

/// Execute judge task on the given submission and returns the judge result.
fn handle_submission(submission: &SubmissionInfo, context: &AppContext)
    -> Result<SubmissionJudgeResult> {
    let problem = context.storage.problems().get(submission.problem_id)?;
    let archive = context.storage.archives().get(problem.archive_id)?;

    if problem.has_jury() && !problem.jury_compile_succeeded() {
        log::error!("the checker of the problem \"{}\" did not compiled successfully.",
            submission.problem_id);
        return Ok(SubmissionJudgeResult::failure("Answer checker did not compiled successfully."));
    }

    // Compile the submission program.
    let compile_result = context.fork_server.compile_source(
        &submission.source,
        submission.language.to_judge_language(),
        judge::CompilationScheme::Judgee)?;
    if !compile_result.succeeded {
        return Ok(SubmissionJudgeResult::compilation_failed(
            compile_result.compiler_out.unwrap_or_default()));
    }

    // Prepare a `JudgeTaskDescriptor`.
    let exec_path = compile_result.compiler_out
        .expect("failed to get the path to the executable file of submission");

    let program = judge::Program::new(exec_path, submission.language.to_judge_language());
    let mut task = judge::JudgeTaskDescriptor::new(program);
    task.limits.cpu_time_limit = Duration::from_millis(problem.time_limit);
    task.limits.real_time_limit = Duration::from_millis(problem.time_limit * 3);
    task.limits.memory_limit = sandbox::MemorySize::MegaBytes(problem.memory_limit as usize);

    task.mode = match problem.judge_mode {
        JudgeMode::Standard => judge::JudgeMode::Standard(judge::BuiltinCheckers::Default),
        JudgeMode::SpecialJudge | JudgeMode::Interactive => {
            let jury_lang = problem.jury_lang.as_ref().unwrap().to_judge_language();
            let jury_exec = problem.jury_exec_path.as_ref().unwrap();
            let jury_program = judge::Program::new(jury_exec, jury_lang);

            if problem.judge_mode == JudgeMode::SpecialJudge {
                judge::JudgeMode::SpecialJudge(jury_program)
            } else { // problem.judge_mode == JudgeMode::SpecialJudge
                judge::JudgeMode::Interactive(jury_program)
            }
        }
    };

    for test_case in archive.test_cases() {
        let test_case_desc = judge::TestCaseDescriptor::new(
            test_case.input_file_path(), test_case.answer_file_path());
        task.test_suite.push(test_case_desc);
    }

    // Execute the judge task.
    let cmd = ForkServerCommand::Judge(task);
    let judge_result = context.fork_server.execute_cmd(&cmd)?.unwrap_as_judge_result();

    Ok(SubmissionJudgeResult::from(judge_result))
}

/// The entry point of a worker thread.
fn worker_entry(worker_id: u32, context: Arc<AppContext>) {
    log::info!("Worker thread #{} has started", worker_id);

    fn sleep_interval() {
        // The interval between two consecutive GET submission requests. The actual interval is
        // determined by adding a randomly generated number between -0.5 and +0.5 to this value.
        const GET_SUBMISSION_INTERVAL: f64 = 3.0;

        let interval = GET_SUBMISSION_INTERVAL + rand::thread_rng().gen::<f64>() - 0.5;
        std::thread::sleep(Duration::from_secs_f64(interval));
    }

    loop {
        let submission = match context.rest.get_submission() {
            Ok(Some(sub)) => sub,
            Ok(None) => {
                sleep_interval();
                continue;
            },
            Err(e) => {
                log::error!("failed to get submission: {}", e);
                sleep_interval();
                continue;
            }
        };

        let result = match handle_submission(&submission, &*context) {
            Ok(r) => {
                log::info!("Judge of submission \"{}\" finished. Verdict: {}",
                    submission.id, r.verdict);
                log::debug!("Judge result detail: {:?}", r);
                r
            },
            Err(e) => {
                log::error!("failed to handle submission \"{}\": {}", submission.id, e);
                SubmissionJudgeResult::failure("")
            }
        };

        let mut retry_count = 3;
        while let Err(e) = context.rest.patch_judge_result(submission.id, &result) {
            log::error!("failed to patch judge result: {}", e);

            retry_count -= 1;
            if retry_count == 0 {
                break;
            }
        }

        if retry_count == 0 {
            log::error!(concat!("failed to patch judge result for submission \"{}\" ",
                "after 3 retries. The judge result will be discarded."), submission.id);
        }

        sleep_interval();
    }
}

/// Spawn and execute worker threads. This function will block until any of the worker threads
/// exits.
pub(crate) fn run(context: Arc<AppContext>) -> Result<()> {
    const MAX_WORKERS: u32 = 10;

    if context.config.workers == 0 {
        log::error!("Number of workers cannot be 0.");
        return Err(Error::from(ErrorKind::InvalidNumberOfWorkers));
    }

    let num_workers = if context.config.workers > MAX_WORKERS {
        log::warn!("Number of workers exceeds maximum limit. Fallback to {} workers.", MAX_WORKERS);
        MAX_WORKERS
    } else {
        context.config.workers
    };

    log::info!("Spawning {} worker threads", num_workers);
    let mut worker_threads: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers as usize);
    for worker_id in 1..=num_workers {
        let context_clone = context.clone();
        let handle = std::thread::spawn(move || worker_entry(worker_id, context_clone));
        worker_threads.push(handle);
    }
    drop(context);

    // Wait for all worker threads to finish.
    for (worker_id, handle) in (1..num_workers).zip(worker_threads) {
        match handle.join() {
            Ok(..) => (),
            Err(e) => {
                log::error!("Worker thread #{} failed.", worker_id);
                return Err(Error::from(ErrorKind::WorkerFailed { worker_id, e }));
            }
        };
    }

    Ok(())
}

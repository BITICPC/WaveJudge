//! This module implements the worker threads.
//!

use std::any::Any;
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::AppContext;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
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

/// The entry point of a worker thread.
fn worker_entry(worker_id: u32, context: Arc<AppContext>) {
    log::info!("Worker thread #{} has started", worker_id);

    unimplemented!()
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

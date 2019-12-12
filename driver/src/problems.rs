//! This module manages problem metadata.
//!

use std::path::PathBuf;

use judge::JudgeMode;

use crate::common::ObjectId;

/// Provide metadata about a problem.
pub struct ProblemMetadata {
    /// The ID of the problem.
    pub id: ObjectId,

    /// The judge mode of the problem.
    pub judge_mode: JudgeMode,

    /// The time limit of the problem, in milliseconds.
    pub time_limit: u64,

    /// The memory limit of the problem, in megabytes.
    pub memory_limit: u64,

    /// Path to the checker's executable, if the `judge_mode` is `JudgeMode::SpecialJudge`.
    pub checker_exec_path: Option<PathBuf>,

    /// Path to the interactor's executable, if the `judge_mode` is `JudgeMode::Interactor`.
    pub interactor_exec_path: Option<PathBuf>,

    /// Timestamp of the last update time of this metadata.
    pub timestamp: u64,
}

// TODO: Implement problems module after forkserver module.

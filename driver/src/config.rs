//! This module maintains application wide configurations.
//!

use std::path::{Path, PathBuf};

use serde::Deserialize;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
        SerdeYamlError(::serde_yaml::Error);
    }

    errors {
        InvalidConfigFile {
            description("invalid config file")
        }
    }
}


/// Provide application wide configurations.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// Number of workers.
    pub workers: u32,

    /// Judge cluster related configurations.
    pub cluster: ClusterConfig,

    /// Storage related configurations.
    pub storage: StorageConfig,

    /// Judge engine related configurations.
    pub engine: JudgeEngineConfig,
}

impl AppConfig {
    /// Load configuration information from the specified file.
    pub fn from_file<P>(path: P) -> Result<Self>
        where P: AsRef<Path> {
        let path = path.as_ref();
        log::info!("Loading application configuration from file: {}", path.display());

        let config_content = std::fs::read_to_string(path)
            .chain_err(|| Error::from(ErrorKind::InvalidConfigFile))
            ?;
        let config: AppConfig = serde_yaml::from_str(&config_content)
            .chain_err(|| Error::from(ErrorKind::InvalidConfigFile))
            ?;

        Ok(config)
    }
}

/// Provide cluster related configurations.
#[derive(Debug, Deserialize)]
pub struct ClusterConfig {
    /// The endpoint of judge board.
    pub judge_board_url: String,

    /// The time interval between two adjacent heartbeat packets.
    pub heartbeat_interval: u32,
}

/// Provide storage related configurations.
#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    /// Path to the database file that contains a sqlite database.
    pub db_file: PathBuf,

    /// The directory under which all test data archives are maintained.
    pub archive_dir: PathBuf,

    /// The directory under which all the compiled jury programs will be maintained.
    pub jury_dir: PathBuf,
}

/// Provide judge engine related configurations.
#[derive(Debug, Deserialize)]
pub struct JudgeEngineConfig {
    /// The directory under which judge tasks will be performed.
    pub judge_dir: PathBuf,

    /// Paths to dynamic linking libraries containing language providers.
    pub language_dylibs: Vec<PathBuf>,

    /// The identity of the user to be used as the effective user of judgees.
    pub judge_username: String,

    /// System call whitelist for the judgee process.
    pub judgee_syscall_whitelist: Vec<String>,

    /// CPU time limit to be applied on the jury (the answer checkers and the interactors), measured
    /// in milliseconds.
    pub jury_cpu_time_limit: u64,

    /// Real time limit to be applied on the jury (the answer checkers and the interactors),
    /// measured in milliseconds.
    pub jury_real_time_limit: u64,

    /// Memory limit to be applied on the jury (the answer checkers and the interactors), measured
    /// in megabytes.
    pub jury_memory_limit: usize,

    /// System call whitelist for the jury (the answer checkers and the interactors) process.
    pub jury_syscall_whitelist: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    #[test]
    fn deserialize_app_config_yaml() {
        let yaml = r#"
            cluster:
                judge_board_url: "http://judge_board"
                heartbeat_interval: 5
            storage:
                archive_dir: "/archive/dir"
                db_file: "path/to/db/file"
            engine:
                judge_dir: "/judge/dir"
                language_dylibs: ["language_dylib_1", "language_dylib_2"]
                judge_username: "Lancern"
                judgee_syscall_whitelist: ["read", "write", "exit"]
                jury_cpu_time_limit: 1000
                jury_real_time_limit: 10000
                jury_memory_limit: 1024
                jury_syscall_whitelist: ["open", "read", "write", "close", "exit"]
        "#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!("http://judge_board", config.cluster.judge_board_url);
        assert_eq!(5, config.cluster.heartbeat_interval);

        assert_eq!(PathBuf::from_str("/archive/dir").unwrap(), config.storage.archive_dir);
        assert_eq!(PathBuf::from_str("path/to/db/file").unwrap(), config.storage.db_file);

        assert_eq!(PathBuf::from_str("/judge/dir").unwrap(), config.engine.judge_dir);
        assert_eq!(vec![PathBuf::from_str("language_dylib_1").unwrap(),
                        PathBuf::from_str("language_dylib_2").unwrap()],
            config.engine.language_dylibs);
        assert_eq!("Lancern", config.engine.judge_username);
        assert_eq!(vec!["read", "write", "exit"], config.engine.judgee_syscall_whitelist);
        assert_eq!(1000, config.engine.jury_cpu_time_limit);
        assert_eq!(10000, config.engine.jury_real_time_limit);
        assert_eq!(1024, config.engine.jury_memory_limit);
        assert_eq!(vec!["open", "read", "write", "close", "exit"],
            config.engine.jury_syscall_whitelist);
    }
}

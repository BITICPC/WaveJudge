//! This module maintains application wide configurations.
//!

use std::path::{Path, PathBuf};

use log::info;
use serde::Deserialize;

use crate::{Error, ErrorKind, ResultExt, Result};

/// Provide application wide configurations.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// The directory under which all test data archives are maintained.
    pub archive_dir: PathBuf,

    /// The directory under which judge tasks will be performed.
    pub judge_dir: PathBuf,

    /// The endpoint of judge board.
    pub judge_board_url: String,

    /// The time interval between two adjacent heartbeat packets.
    pub heartbeat_interval: u32,

    /// Paths to dynamic linking libraries containing language providers.
    pub language_dylibs: Vec<PathBuf>,
}

/// The application wide singleton object of application configuration.
static mut SINGLETON: Option<AppConfig> = None;

/// Get an `AppConfig` value containing application wide configurations. This function panics if
/// the configuration has not been initialized.
pub fn app_config() -> &'static AppConfig {
    unsafe {
        SINGLETON.as_ref().unwrap()
    }
}

/// Initialize configuration from the specified file. This function panics if the configuration has
/// already been initialized.
pub fn init_config<T: AsRef<Path>>(config_file: T) -> Result<()> {
    info!("Initializing application configuration from file: {}", config_file.as_ref().display());

    let config_content = std::fs::read_to_string(config_file)
        .chain_err(|| Error::from(ErrorKind::InvalidConfigFile))
        ?;
    let config: AppConfig = serde_json::from_str(&config_content)
        .chain_err(|| Error::from(ErrorKind::InvalidConfigFile))
        ?;

    unsafe {
        SINGLETON.replace(config).unwrap();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    #[test]
    fn deserialize_app_config_json() {
        let json = r#"
            {
                "archive_dir": "/archive/dir",
                "judge_dir": "/judge/dir",
                "judge_board_url": "http://judge.board",
                "heartbeat_interval": 5,
                "language_dylibs": [
                    "language_dylib_1",
                    "language_dylib_2"
                ]
            }
        "#;
        let config: AppConfig = serde_json::from_str(json).unwrap();

        assert_eq!(PathBuf::from_str("/archive/dir").unwrap(), config.archive_dir);
        assert_eq!(PathBuf::from_str("/judge/dir").unwrap(), config.judge_dir);
        assert_eq!("http://judge.board", config.judge_board_url);
        assert_eq!(5, config.heartbeat_interval);
        assert_eq!(vec![PathBuf::from_str("language_dylib_1").unwrap(),
                        PathBuf::from_str("language_dylib_2").unwrap()],
                   config.language_dylibs);
    }
}

//! This module is responsible of the initialization of the application.
//!

use std::path::Path;
use std::sync::Arc;

use openssl::rsa::Rsa;

use clap::ArgMatches;

use crate::AppContext;

use crate::config::AppConfig;
use crate::forkserver::ForkServerClient;
use crate::storage::AppStorageFacade;
use crate::restful::RestfulClient;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        LogError(::log4rs::Error);
        IoError(::std::io::Error);
        OpenSslError(::openssl::error::ErrorStack);
        UrlError(::reqwest::UrlError);
    }

    links {
        ConfigError(crate::config::Error, crate::config::ErrorKind);
        ForkServerError(crate::forkserver::Error, crate::forkserver::ErrorKind);
        StorageError(crate::storage::Error, crate::storage::ErrorKind);
    }
}

/// Provide a builder for `AppContext` values.
struct AppContextBuilder {
    /// The application wide configuration.
    config: Option<Arc<AppConfig>>,

    /// The fork server client.
    fork_server: Option<Arc<ForkServerClient>>,

    /// The REST client connected to the judge board server.
    rest: Option<Arc<RestfulClient>>,

    /// The application storage facade.
    storage: Option<AppStorageFacade>,
}

impl AppContextBuilder {
    /// Create a new `AppContextBuilder` instance.
    fn new() -> Self {
        AppContextBuilder {
            config: None,
            fork_server: None,
            rest: None,
            storage: None,
        }
    }

    /// Initialize application wide configuration and populate the `config` field.
    fn init_app_config<P>(&mut self, config_file: P) -> Result<()>
        where P: AsRef<Path> {
        let config_file = config_file.as_ref();
        log::info!("Initializing application configuration from file {}", config_file.display());

        let config = AppConfig::from_file(config_file)?;
        self.config = Some(Arc::new(config));
        Ok(())
    }

    /// Get a reference to the application wide configuration. This function panics if the
    /// application wide configuration has not been initialized yet.
    fn get_app_config(&self) -> &AppConfig {
        &*self.config.as_ref().expect("Application configuration has not been initialized yet.")
    }

    /// Initialize fork sevrer.
    fn init_fork_server(&mut self) -> Result<()> {
        let judge_config = &self.get_app_config().engine;
        let client = crate::forkserver::start_fork_server(judge_config)?;
        self.fork_server = Some(Arc::new(client));
        Ok(())
    }

    /// Get an Arc to the initialized fork server. This function panics if the fork server has not
    /// been initialized yet.s
    fn get_fork_server(&self) -> Arc<ForkServerClient> {
        self.fork_server.as_ref()
            .expect("fork server has not been initialized yet.")
            .clone()
    }

    /// Initialize RESTful client to the judge board server.
    fn init_rest(&mut self) -> Result<()> {
        let config = &self.get_app_config().cluster;
        let judge_board_url = config.judge_board_url.clone();
        log::info!("Initializing REST client with judge board at {}", judge_board_url);

        let judge_board_url = match reqwest::Url::parse(&judge_board_url) {
            Ok(url) => url,
            Err(e) => {
                log::error!("Failed to parse judge board URL: {}", e);
                return Err(Error::from(e));
            }
        };

        log::debug!("Loading authenticate key from PEM file: \"{}\"",
            config.authenticate_key_file.display());
        let pem_data = match std::fs::read(&config.authenticate_key_file) {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to load authenticate key from file \"{}\": {}",
                    config.authenticate_key_file.display(), e);
                return Err(Error::from(e));
            }
        };
        let auth_key = Rsa::private_key_from_pem(&pem_data)?;

        let rest = RestfulClient::new(judge_board_url, auth_key);
        self.rest = Some(Arc::new(rest));

        Ok(())
    }

    /// Get an Arc to the initialized RESTful client object. This function panics if the RESTful
    /// client has not been initialized yet.
    fn get_rest(&self) -> Arc<RestfulClient> {
        self.rest.as_ref()
            .expect("RESTful client has not been initialized yet.")
            .clone()
    }

    /// Initialize application storage facade.
    fn init_storage_facade(&mut self) -> Result<()> {
        log::info!("Initializing application storage facilities");

        let config = self.get_app_config();
        let rest = self.get_rest();
        let fork_server = self.get_fork_server();
        let storage = AppStorageFacade::new(&config, rest, fork_server)?;

        self.storage = Some(storage);
        Ok(())
    }

    /// Initialize all components. `config_path` is the path to the application wide configuration
    /// file.
    fn init_all<P>(&mut self, config_path: P) -> Result<()>
        where P: AsRef<Path> {
        self.init_app_config(config_path)?;
        // The initialization of fork server should be as early as possible to avoid unnecessary
        // memory footprint in the fork server process.
        self.init_fork_server()?;
        self.init_rest()?;
        self.init_storage_facade()?;

        Ok(())
    }

    /// Build `AppContext` object and consume the current `AppContextBuilder` object. This function
    /// panics if some of the fields in this `AppContextBuilder` has not been initialized yet.
    fn build_app_context(self) -> AppContext {
        AppContext {
            config: self.config.expect("Application configuration has not been initialized yet."),
            fork_server: self.fork_server.expect("Fork server has not been initialized yet."),
            rest: self.rest.expect("RESTful client has not been initialized yet."),
            storage: self.storage.expect("Application storage has not been initialized yet."),
        }
    }
}

/// Initialize log facilities. `log_config_file` is the path to the log configuration file.
fn init_log<P>(log_config_file: P) -> Result<()>
    where P: AsRef<Path> {
    log4rs::init_file(log_config_file, log4rs::file::Deserializers::default())?;
    Ok(())
}

/// Initialize the application and returns a `AppContext` object.
pub(crate) fn init<'a>(args: ArgMatches<'a>) -> Result<AppContext> {
    let log_config_file_path = args.value_of("log_config_file")
        .expect("failed to get path to log file");
    init_log(log_config_file_path)?;

    let mut builder = AppContextBuilder::new();

    let config_file = args.value_of("config_file")
        .expect("failed to get path to the configuration file");
    builder.init_all(config_file)?;

    Ok(builder.build_app_context())
}

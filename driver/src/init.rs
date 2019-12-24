//! This module is responsible of the initialization of the application.
//!

use std::path::Path;
use std::sync::Arc;

use clap::ArgMatches;

use crate::AppContext;

use crate::archives::ArchiveStore;
use crate::config::AppConfig;
use crate::db::SqliteConnection;
use crate::forkserver::ForkServerClient;
use crate::problems::ProblemStore;
use crate::restful::RestfulClient;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        LogError(::log4rs::Error);
    }

    links {
        ConfigError(crate::config::Error, crate::config::ErrorKind);
        SqliteError(crate::db::Error, crate::db::ErrorKind);
        ForkServerError(crate::forkserver::Error, crate::forkserver::ErrorKind);
    }
}

/// Provide a builder for `AppContext` values.
struct AppContextBuilder {
    /// The application wide configuration.
    config: Option<Arc<AppConfig>>,

    /// The fork server client.
    fork_server: Option<Arc<ForkServerClient>>,

    /// The connection to the sqlite database.
    db: Option<Arc<SqliteConnection>>,

    /// The REST client connected to the judge board server.
    rest: Option<Arc<RestfulClient>>,

    /// The archive store.
    archives: Option<ArchiveStore>,

    /// The problem store.
    problems: Option<ProblemStore>,
}

impl AppContextBuilder {
    /// Create a new `AppContextBuilder` instance.
    fn new() -> Self {
        AppContextBuilder {
            config: None,
            db: None,
            archives: None,
            problems: None,
            rest: None,
            fork_server: None,
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

    /// Initialize the database connection to the sqlite database.
    fn init_db(&mut self) -> Result<()> {
        let db_file = &self.get_app_config().storage.db_file;
        log::info!("Initializing database connection to sqlite instance at {}", db_file.display());

        let conn = SqliteConnection::new(db_file)?;
        self.db = Some(Arc::new(conn));
        Ok(())
    }

    /// Get an Arc to the initialized database connection object. This function panics if the
    /// database connection has not been initialized yet.
    fn get_db(&self) -> Arc<SqliteConnection> {
        self.db.as_ref()
            .expect("Sqlite database connection has not been initialized yet.")
            .clone()
    }

    /// Initialize RESTful client to the judge board server.
    fn init_rest(&mut self) {
        let judge_board_url = &self.get_app_config().cluster.judge_board_url;
        log::info!("Initializing REST client with judge board at {}", judge_board_url);

        let rest = RestfulClient::new(judge_board_url);
        self.rest = Some(Arc::new(rest));
    }

    /// Get an Arc to the initialized RESTful client object. This function panics if the RESTful
    /// client has not been initialized yet.
    fn get_rest(&self) -> Arc<RestfulClient> {
        self.rest.as_ref()
            .expect("RESTful client has not been initialized yet.")
            .clone()
    }

    /// Initialize test archive cache store.
    fn init_archives(&mut self) -> Result<()> {
        let archive_dir = &self.get_app_config().storage.archive_dir;
        log::info!("Initializing test archive store at {}", archive_dir.display());

        let rest = self.rest.as_ref().expect("RESTful client has not been initialized yet.")
            .clone();
        let store = ArchiveStore::new(archive_dir, rest);
        self.archives = Some(store);
        Ok(())
    }

    /// Initialize problems metadata store.
    fn init_problems(&mut self) -> Result<()> {
        log::info!("Initializing problem store");

        let db = self.get_db();
        let rest = self.get_rest();
        let store = ProblemStore::new(db, rest);
        self.problems = Some(store);

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
        self.init_db()?;
        self.init_rest();
        self.init_archives()?;
        self.init_problems()?;

        Ok(())
    }

    /// Build `AppContext` object and consume the current `AppContextBuilder` object. This function
    /// panics if some of the fields in this `AppContextBuilder` has not been initialized yet.
    fn build_app_context(self) -> AppContext {
        AppContext {
            config: self.config.expect("Application configuration has not been initialized yet."),
            fork_server: self.fork_server.expect("Fork server has not been initialized yet."),
            db: self.db.expect("Sqlite database connection has not been initialized yet."),
            rest: self.rest.expect("RESTful client has not been initialized yet."),
            archives: self.archives.expect("Test archive store has not been initialized yet."),
            problems: self.problems.expect("Problem metadata store has not been initialized yet."),
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

extern crate log;
extern crate log4rs;
extern crate error_chain;
extern crate libc;
extern crate nix;
extern crate rand;
extern crate sqlite;
extern crate procfs;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;
extern crate rmp_serde;
extern crate zip;
extern crate tempfile;
extern crate clap;

extern crate judge;
extern crate sandbox;

mod config;
mod forkserver;
mod heartbeat;
mod init;
mod restful;
mod storage;
mod utils;
mod workers;

use std::sync::Arc;
use std::time::Duration;

use config::AppConfig;
use forkserver::ForkServerClient;
use heartbeat::HeartbeatDaemonOptions;
use restful::RestfulClient;
use storage::AppStorageFacade;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        InitializationError(init::Error, init::ErrorKind);
        WorkerError(workers::Error, workers::ErrorKind);
    }
}

/// Provide application wide context for worker threads.
struct AppContext {
    /// The application wide configuration.
    config: Arc<AppConfig>,

    /// The fork server client.
    fork_server: Arc<ForkServerClient>,

    /// The RESTful client.
    rest: Arc<RestfulClient>,

    /// The storage facade of this application.
    storage: AppStorageFacade,
}

fn do_main() -> Result<()> {
    let arg_matches = clap::App::new("wave_judge")
        .version("1.0")
        .author("Lancern <msrlancern@126.com>")
        .about("Wave judge system judge node application")
        .arg(clap::Arg::with_name("log_config_file")
            .long("logconfig")
            .value_name("LOG_CONFIG_FILE")
            .help("Set the path to the log configuration file")
            .takes_value(true)
            .required(false)
            .default_value("config/log-config.yaml"))
        .arg(clap::Arg::with_name("config_file")
            .short("c")
            .long("config")
            .value_name("CONFIG_FILE")
            .help("Set the path to the configuration file")
            .takes_value(true)
            .required(false)
            .default_value("config/app.yaml"))
        .get_matches();
    let context = init::init(arg_matches)?;

    // Start heartbeat daemon threads.
    let hb_options = HeartbeatDaemonOptions::new(
        context.rest.clone(),
        Duration::from_secs(context.config.cluster.heartbeat_interval as u64));
    heartbeat::start_daemon(hb_options);

    workers::run(Arc::new(context))?;
    Ok(())
}

fn main() {
    match do_main() {
        Err(e) => {
            eprintln!("FATAL ERROR: {}", e);
            std::process::exit(-1);
        },
        _ => ()
    };
}

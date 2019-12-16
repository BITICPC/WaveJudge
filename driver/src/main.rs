extern crate log;
extern crate log4rs;
extern crate error_chain;
extern crate libc;
extern crate nix;
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

mod archives;
mod common;
mod config;
mod db;
mod forkserver;
mod heartbeat;
mod restful;
mod problems;
mod utils;

use std::path::Path;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
        SerdeYamlError(::serde_yaml::Error);
        LogError(::log4rs::Error);
    }

    errors {
        InvalidConfigFile {
            description("invalid config file")
        }
    }
}

fn init_log<P>(log_config_file: P) -> Result<()>
    where P: AsRef<Path> {
    log4rs::init_file(log_config_file, log4rs::file::Deserializers::default())?;
    Ok(())
}

fn init_app_config<P>(config_file: P) -> Result<()>
    where P: AsRef<Path> {
    crate::config::init_config(config_file)?;
    Ok(())
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

    let log_config_file_path = arg_matches.value_of("log_config_file")
        .expect("failed to get path to log file");
    init_log(log_config_file_path)?;

    let config_file = arg_matches.value_of("config_file")
        .expect("failed to get path to the configuration file");
    init_app_config(config_file)?;

    // TODO: Implement do_main().
    unimplemented!()
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

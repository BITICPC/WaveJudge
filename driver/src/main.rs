extern crate log;
extern crate error_chain;
extern crate libc;
extern crate nix;
extern crate procfs;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate zip;
extern crate tempfile;
extern crate clap;

extern crate judge;
extern crate sandbox;

mod archives;
mod config;
mod heartbeat;
mod restful;
mod utils;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
        SerdeJsonError(::serde_json::Error);
    }

    errors {
        InvalidConfigFile {
            description("invalid config file")
        }
    }
}

fn main() {
    unimplemented!()
}

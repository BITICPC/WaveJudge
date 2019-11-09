#![cfg(unix)]

#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate nix;
extern crate seccomp_sys;
extern crate procinfo;

mod sandbox;

mod errors {
    error_chain!{
        types {
            Error, ErrorKind, ResultExt, Result;
        }

        links {
            Sandbox(crate::sandbox::Error, crate::sandbox::ErrorKind);
        }
    }
}

fn main() {
    println!("Hello, world!");
}

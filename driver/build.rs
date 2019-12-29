use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=config");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::env::var("OUT_DIR")
        .expect("Failed to get OUT_DIR environment variable.");
    let out_dir = PathBuf::from(out_dir);
    eprintln!("OUT_DIR={}", out_dir.display());

    if !out_dir.exists() {
        std::fs::create_dir_all(&out_dir)
            .expect("Failed to create output directory");
    }

    copy_config_files(&out_dir);
}

fn copy_config_files<P>(out_dir: &P)
    where P: ?Sized + AsRef<Path> {
    let out_dir = out_dir.as_ref().to_owned();

    // Copy all configuration files under config/ to the output directory.
    let config_dir = PathBuf::from("./config");
    let read_config_dir = config_dir.read_dir()
        .expect("Failed to read content of config directory.");

    for config_file in read_config_dir {
        let config_file = config_file.expect("failed to iterate content of config directory.");
        let config_file = config_file.path();

        let config_file_name = config_file.file_name().unwrap();
        let mut target_config_file = out_dir.clone();
        target_config_file.push(config_file_name);

        eprintln!("Copying config file \"{}\" to output directory", config_file.display());
        std::fs::copy(config_file, target_config_file)
            .expect("Failed to copy config file.");
    }
}

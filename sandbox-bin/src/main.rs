#[macro_use]
extern crate error_chain;
extern crate sandbox;
extern crate clap;

use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use error_chain::ChainedError;

use sandbox::{
    MemorySize,
    UserId,
    SystemCall,
    ProcessBuilder,
    ProcessExitStatus
};


error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        Sandbox(sandbox::Error, sandbox::ErrorKind);
    }

    foreign_links {
        Io(::std::io::Error);
        Clap(::clap::Error);
    }
}


struct ApplicationConfig {
    pub file: PathBuf,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,

    pub cpu_time_limit: Option<Duration>,
    pub real_time_limit: Option<Duration>,
    pub memory_limit: Option<MemorySize>,

    pub input_file: Option<PathBuf>,
    pub output_file: Option<PathBuf>,
    pub error_file: Option<PathBuf>,

    pub uid: Option<UserId>,
    pub syscall_whitelist: Vec<SystemCall>
}

impl ApplicationConfig {
    fn new() -> ApplicationConfig {
        ApplicationConfig {
            file: PathBuf::new(),
            args: Vec::new(),
            envs: Vec::new(),
            cpu_time_limit: None,
            real_time_limit: None,
            memory_limit: None,
            input_file: None,
            output_file: None,
            error_file: None,
            uid: None,
            syscall_whitelist: Vec::new()
        }
    }
}

fn get_app_config() -> Result<ApplicationConfig> {
    let matches = clap::App::new("Wave Judge Sandbox Wrapper")
        .version("0.1")
        .author("Lancern <msrlancern@126.com>")
        .about("Wrapper program for the wave judge sandbox component")
        .arg(clap::Arg::with_name("cpu_time_limit")
            .short("t")
            .long("cpu")
            .takes_value(true)
            .value_name("CPU_TIME_LIMIT")
            .help("specify the CPU time limit, in milliseconds"))
        .arg(clap::Arg::with_name("real_time_limit")
            .short("r")
            .long("real")
            .takes_value(true)
            .value_name("REAL_TIME_LIMIT")
            .help("specify the real time limit, in milliseconds"))
        .arg(clap::Arg::with_name("memory_limit")
            .short("m")
            .long("mem")
            .takes_value(true)
            .value_name("MEMORY_LIMIT")
            .help("specify the memory limit, in megabytes."))
        .arg(clap::Arg::with_name("input_file")
            .short("i")
            .long("input")
            .takes_value(true)
            .value_name("INPUT_FILE")
            .help("specify the path to the input file"))
        .arg(clap::Arg::with_name("output_file")
            .short("o")
            .long("output")
            .takes_value(true)
            .value_name("OUTPUT_FILE")
            .help("specify the path to the output file"))
        .arg(clap::Arg::with_name("error_file")
            .short("e")
            .long("error")
            .takes_value(true)
            .value_name("ERROR_FILE")
            .help("specify the path to the error file"))
        .arg(clap::Arg::with_name("uid")
            .short("u")
            .long("uid")
            .takes_value(true)
            .value_name("UID")
            .help("specify the effective uid of the sandbox process"))
        .arg(clap::Arg::with_name("syscall_whitelist")
            .short("s")
            .long("syscall")
            .takes_value(true)
            .value_name("ALLOWED_SYSCALL_NAMEs")
            .multiple(true)
            .value_terminator("--")
            .help("specify the names of allowed system call"))
        .arg(clap::Arg::with_name("envs")
            .long("env")
            .takes_value(true)
            .value_name("ENVs")
            .multiple(true)
            .help("specify the environment variables passed to the child process"))
        .arg(clap::Arg::with_name("program")
            .value_name("PROGRAM")
            .takes_value(true)
            .multiple(true)
            .required(true)
            .help("specify the program along with its arguments"))
        .get_matches();

    let mut config = ApplicationConfig::new();

    let program = matches.values_of("program").unwrap().collect::<Vec<&'_ str>>();
    config.file = PathBuf::from_str(program[0]).unwrap();
    for arg in &program[1..] {
        config.args.push((*arg).to_owned());
    }

    match matches.values_of("envs") {
        Some(arg_envs) => {
            for envs in arg_envs {
                if !envs.contains('=') {
                    return Err(Error::from(format!("invalid environment variable: {}", envs)));
                }

                let (name, value) = envs.split_at(envs.find('=').unwrap());
                config.envs.push((name.to_owned(), value.to_owned()));
            }
        },
        None => ()
    };

    match matches.value_of("cpu_time_limit") {
        Some(cpu_limit) => {
            let cpu_limit = u64::from_str(cpu_limit)
                .chain_err(|| Error::from(format!("invalid cpu limit value: {}", cpu_limit)))
                ?;
            config.cpu_time_limit = Some(Duration::from_millis(cpu_limit));
        },
        None => ()
    };

    match matches.value_of("real_time_limit") {
        Some(real_limit) => {
            let real_limit = u64::from_str(real_limit)
                .chain_err(|| Error::from(format!("invalid real time limit value: {}", real_limit)))
                ?;
            config.real_time_limit = Some(Duration::from_millis(real_limit));
        },
        None => ()
    };

    match matches.value_of("memory_limit") {
        Some(mem_limit) => {
            let mem_limit = usize::from_str(mem_limit)
                .chain_err(|| Error::from(format!("invalid memory limit value: {}", mem_limit)))
                ?;
            config.memory_limit = Some(MemorySize::MegaBytes(mem_limit));
        },
        None => ()
    };

    config.input_file = matches.value_of("input_file")
        .map(|f| PathBuf::from_str(f).unwrap());
    config.output_file = matches.value_of("output_file")
        .map(|f| PathBuf::from_str(f).unwrap());
    config.error_file = matches.value_of("error_file")
        .map(|f| PathBuf::from_str(f).unwrap());

    match matches.value_of("uid") {
        Some(uid) => {
            let uid = UserId::from_str(uid)
                .chain_err(|| Error::from(format!("invalid user ID value: {}", uid)))
                ?;
            config.uid = Some(uid);
        },
        None => ()
    };

    match matches.values_of("syscall_whitelist") {
        Some(syscalls) => {
            for syscall in syscalls {
                config.syscall_whitelist.push(SystemCall::from_name(syscall)?);
            }
        },
        None => ()
    };

    Ok(config)
}

fn do_main() -> Result<()> {
    let config = get_app_config()?;

    let mut builder = ProcessBuilder::new(&config.file);
    for arg in &config.args {
        builder.add_arg(arg)?;
    }
    for (name, value) in &config.envs {
        builder.add_env(name, value)?;
    }

    builder.limits.cpu_time_limit = config.cpu_time_limit;
    builder.limits.real_time_limit = config.real_time_limit;
    builder.limits.memory_limit = config.memory_limit;

    if config.input_file.is_some() {
        builder.redirections.stdin = Some(File::open(config.input_file.unwrap())
            .chain_err(|| Error::from("cannot open input file"))
            ?);
    }

    if config.output_file.is_some() {
        builder.redirections.stdout = Some(File::create(config.output_file.unwrap())
            .chain_err(|| Error::from("cannot open output file"))
            ?);
    }

    if config.error_file.is_some() {
        builder.redirections.stderr = Some(File::create(config.error_file.unwrap())
            .chain_err(|| Error::from("cannot open error file"))
            ?);
    }

    builder.uid = config.uid;
    for syscall in config.syscall_whitelist {
        builder.allow_syscall(syscall);
    }

    let mut process = builder.start()?;
    process.wait_for_exit()?;

    let exit_status = process.exit_status();
    print!("Process exited: ");
    match exit_status {
        ProcessExitStatus::Normal(exit_code) =>
            println!("normal, exit code = {}", exit_code),
        ProcessExitStatus::KilledBySignal(signal) =>
            println!("kill by signal: {}", signal),
        ProcessExitStatus::CPUTimeLimitExceeded =>
            println!("cpu time limit exceeded"),
        ProcessExitStatus::MemoryLimitExceeded =>
            println!("memory limit exceeded"),
        ProcessExitStatus::RealTimeLimitExceeded =>
            println!("real time limit exceeded"),
        ProcessExitStatus::BannedSyscall =>
            println!("banned system call"),
        _ => unreachable!()
    };

    let rusage = process.rusage();
    println!("Process resource usage:");
    println!("\tUser CPU time: {} ms", rusage.user_cpu_time.as_millis());
    println!("\tKernel CPU time: {} ms", rusage.kernel_cpu_time.as_millis());
    println!("\tResident set size: {} bytes", rusage.resident_set_size.bytes());
    println!("\tVirtual memory size: {} bytes", rusage.virtual_mem_size.bytes());

    Ok(())
}

fn main() -> Result<()> {
    match do_main() {
        Ok(..) => Ok(()),
        Err(e) => {
            eprintln!("error: {}", e.display_chain().to_string());
            Err(e)
        }
    }
}
